use syn::visit::Visit;

fn main() {
	//assert_eq!(std::env::var("TARGET").unwrap(), "i686-pc-windows-msvc");

	let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
	let out_dir = std::env::var("OUT_DIR").unwrap();

	println!("cargo::rustc-link-lib=EuroScopePlugInDll");
	println!("cargo::rustc-link-search={crate_dir}/ext/euroscope/lib/");

	cc::Build::new()
		.cpp(true)
		.files(
			std::fs::read_dir(format!("{crate_dir}/src"))
				.unwrap()
				.filter_map(|entry| entry.ok())
				.filter(|entry| {
					entry
						.file_name()
						.to_str()
						.is_some_and(|name| name.ends_with(".cpp"))
				})
				.map(|entry| entry.path()),
		)
		.include(format!("{crate_dir}/ext/euroscope/inc/"))
		.compile(&env!("CARGO_PKG_NAME"));

	let bindings = bindgen::builder()
		.header("src/lib.hpp")
		.parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
		.clang_arg(format!("-I{crate_dir}/ext/euroscope/inc/"))
		.clang_arg("-Wno-microsoft")
		.enable_cxx_namespaces()
		.generate()
		.unwrap()
		.to_string();

	std::fs::write(format!("{out_dir}/bindings.rs"), &bindings).unwrap();

	let mut visitor = Visitor {
		out_dir: format!("{out_dir}/mangled-name"),
	};
	std::fs::create_dir_all(&visitor.out_dir).unwrap();
	visitor.visit_file(&syn::parse_str::<syn::File>(&bindings).unwrap());
}

struct Visitor {
	out_dir: String,
}

impl Visit<'_> for Visitor {
	fn visit_foreign_item_fn(&mut self, item: &'_ syn::ForeignItemFn) {
		for attr in &item.attrs {
			if let syn::Meta::NameValue(syn::MetaNameValue {
				path,
				value:
					syn::Expr::Lit(syn::ExprLit {
						lit: syn::Lit::Str(value),
						..
					}),
				..
			}) = &attr.meta
				&& path.is_ident("link_name")
			{
				std::fs::write(
					format!("{}/{}", self.out_dir, item.sig.ident),
					format!("{:?}", value.value().replace("\u{1}", "")),
				)
				.unwrap();
			}
		}
	}
}
