fn main() -> anyhow::Result<()> {
    skilvella::update::release::run(std::env::args_os().skip(1))
}
