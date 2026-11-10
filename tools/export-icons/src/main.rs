use anyhow::{Context, Result, bail, ensure};
use image::{
    ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, Rgba, Rgba32FImage, RgbaImage,
    codecs::png::PngEncoder,
    imageops::{FilterType, resize},
};
use std::{
    env,
    fs::{self, Metadata},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const SIZES: [u32; 11] = [16, 20, 24, 32, 48, 64, 128, 180, 256, 512, 1024];
const USAGE: &str = "Usage: skilvella-export-icons [--source PNG] [--output-dir DIRECTORY]";

fn main() -> Result<()> {
    let defaults = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../static/icons");
    let mut source = defaults.join("skilvella.png");
    let mut output = defaults;
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let path = match arg.to_str() {
            Some("--source") => &mut source,
            Some("--output-dir") => &mut output,
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => bail!("Unknown argument: {arg:?}\n{USAGE}"),
        };
        *path = args
            .next()
            .with_context(|| format!("Missing path for {arg:?}"))?
            .into();
    }
    export(&source, &output)
}

fn export(source: &Path, output: &Path) -> Result<()> {
    let source = checked_path(source)?;
    let output = checked_path(output)?;
    ensure!(
        source.is_file(),
        "Source is not a file: {}",
        source.display()
    );
    ensure!(
        output.is_dir(),
        "Output is not a directory: {}",
        output.display()
    );

    let targets: Vec<_> = SIZES
        .iter()
        .map(|size| output.join(format!("icon-{size}.png")))
        .chain([output.join("skilvella.ico")])
        .collect();
    for target in &targets {
        let Some(metadata) = output_metadata(target)? else {
            continue;
        };
        ensure!(
            !metadata.permissions().readonly(),
            "Read-only output: {}",
            target.display()
        );
        ensure!(
            fs::canonicalize(target)? != source,
            "Output would overwrite the source: {}",
            target.display()
        );
    }

    let master = ImageReader::with_format(
        std::io::BufReader::new(
            fs::File::open(&source).with_context(|| format!("Open {}", source.display()))?,
        ),
        ImageFormat::Png,
    )
    .decode()
    .with_context(|| format!("Decode {}", source.display()))?
    .into_rgba8();
    let frames = png_frames(&master)?;
    let ico = encode_ico(&frames)?;

    // 全ファイルを書き出してから公開する。公開時に失敗しても各ファイルは旧版か完成版のどちらかになる。
    let staged = frames
        .iter()
        .map(|(_, png)| png.as_slice())
        .chain([ico.as_slice()])
        .map(|bytes| -> Result<_> {
            let mut file = NamedTempFile::new_in(&output)
                .with_context(|| format!("Stage output in {}", output.display()))?;
            file.write_all(bytes)?;
            file.as_file().sync_all()?;
            Ok(file)
        })
        .collect::<Result<Vec<_>>>()?;
    for (target, file) in targets.iter().zip(staged) {
        output_metadata(target)?;
        file.persist(target)
            .with_context(|| format!("Replace {}", target.display()))?;
    }
    Ok(())
}

fn checked_path(path: &Path) -> Result<PathBuf> {
    let path = std::path::absolute(path)?;
    // マスターと出力先の親階層も確認し、ジャンクション経由の書き出しを避ける。
    for entry in path.ancestors() {
        let metadata =
            fs::symlink_metadata(entry).with_context(|| format!("Inspect {}", entry.display()))?;
        ensure!(
            !is_link(&metadata),
            "Link or reparse point: {}",
            entry.display()
        );
    }
    fs::canonicalize(&path).with_context(|| format!("Resolve {}", path.display()))
}

fn output_metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !is_link(&metadata),
                "Output is not a regular file: {}",
                path.display()
            );
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Inspect {}", path.display())),
    }
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.is_symlink()
    }
}

fn png_frames(master: &RgbaImage) -> Result<Vec<(u32, Vec<u8>)>> {
    ensure!(
        master.width() == master.height(),
        "A square source image is required"
    );

    // 線形光に変換し、透過部分のRGBが境界ににじまないようアルファを乗算してからリサイズする。
    let linear = Rgba32FImage::from_fn(master.width(), master.height(), |x, y| {
        let [r, g, b, a] = master.get_pixel(x, y).0;
        let alpha = f32::from(a) / 255.0;
        let [r, g, b] = [r, g, b].map(|channel| {
            let srgb = f32::from(channel) / 255.0;
            alpha
                * if srgb <= 0.04045 {
                    srgb / 12.92
                } else {
                    ((srgb + 0.055) / 1.055).powf(2.4)
                }
        });
        Rgba([r, g, b, alpha])
    });

    SIZES
        .into_iter()
        .map(|size| {
            let scaled = resize(&linear, size, size, FilterType::CatmullRom);
            let rgba = RgbaImage::from_fn(size, size, |x, y| {
                let [r, g, b, alpha] = scaled.get_pixel(x, y).0;
                let a = (alpha * 255.0).round() as u8;
                if a == 0 {
                    return Rgba([0; 4]);
                }
                let [r, g, b] = [r, g, b].map(|channel| {
                    let linear = (channel / alpha).clamp(0.0, 1.0);
                    let srgb = if linear <= 0.0031308 {
                        12.92 * linear
                    } else {
                        1.055 * linear.powf(1.0 / 2.4) - 0.055
                    };
                    (srgb * 255.0).round() as u8
                });
                Rgba([r, g, b, a])
            });
            let mut png = Vec::new();
            PngEncoder::new(&mut png).write_image(
                rgba.as_raw(),
                size,
                size,
                ExtendedColorType::Rgba8,
            )?;
            Ok((size, png))
        })
        .collect()
}

fn encode_ico(frames: &[(u32, Vec<u8>)]) -> Result<Vec<u8>> {
    let entries: Vec<_> = frames
        .iter()
        .filter(|(size, _)| *size <= 256 && *size != 180)
        .collect();
    let count = u16::try_from(entries.len())?;
    let mut ico = Vec::new();
    ico.extend([0, 0, 1, 0]); // ICO ヘッダーの予約領域と画像種別。
    ico.extend(count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for (size, png) in &entries {
        // ICOの寸法欄では256pxを0として表す。PNGは書き出したものと同じバイト列を格納する。
        let dim = if *size == 256 {
            0
        } else {
            u8::try_from(*size)?
        };
        ico.extend([dim, dim, 0, 0, 1, 0, 32, 0]);
        let length = u32::try_from(png.len())?;
        ico.extend(length.to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset = offset.checked_add(length).context("ICO is too large")?;
    }
    for (_, png) in entries {
        ico.extend(png);
    }
    Ok(ico)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_and_ico_keep_the_export_sizes_and_alpha() {
        // 透明な青と不透明な赤の境界で、青が半透明画素へ混ざらないことも検証する。
        let master = RgbaImage::from_fn(2, 2, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 0])
            }
        });
        let frames = png_frames(&master).unwrap();
        assert_eq!(
            frames.iter().map(|(size, _)| *size).collect::<Vec<_>>(),
            SIZES
        );
        for (size, png) in &frames {
            let decoded = image::load_from_memory_with_format(png, ImageFormat::Png)
                .unwrap()
                .into_rgba8();
            assert_eq!(decoded.dimensions(), (*size, *size));
            let edge = decoded.get_pixel(size / 2, size / 2).0;
            assert_eq!(&edge[..3], &[255, 0, 0]);
            assert!((1..255).contains(&edge[3]));
        }

        let ico = encode_ico(&frames).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 8, 0]);
        let mut expected_offset = 6 + 16 * 8;
        for (index, size) in [16, 20, 24, 32, 48, 64, 128, 256].into_iter().enumerate() {
            let entry = &ico[6 + index * 16..][..16];
            let dim = if size == 256 { 0 } else { size as u8 };
            assert_eq!(&entry[..8], &[dim, dim, 0, 0, 1, 0, 32, 0]);
            let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            assert_eq!(offset, expected_offset);
            let (_, png) = frames.iter().find(|(frame, _)| *frame == size).unwrap();
            assert_eq!(&ico[offset..offset + length], png);
            expected_offset += length;
        }
        assert_eq!(expected_offset, ico.len());
        assert!(png_frames(&RgbaImage::new(2, 1)).is_err());
    }
}
