fn main() -> anyhow::Result<()> {
	println!("{:#?}", bars_config::Config::load(std::io::stdin())?);
	Ok(())
}
