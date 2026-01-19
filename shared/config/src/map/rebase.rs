use super::*;

impl Maps {
	pub fn rebase(
		&mut self,
		aerodrome: &Aerodrome,
		styles: usize,
	) -> Result<(), &str> {
		use std::collections::HashMap;

		macro_rules! make_lookup {
			($($field:ident),*) => {
				Lookup {
					$(
						$field: {
							let lookup = aerodrome
								.$field
								.iter()
								.enumerate()
								.map(|(i, item)| (&item.id, i))
								.collect::<HashMap<_, _>>();
							self
								.$field
								.iter()
								.map(|id| {
									lookup
										.get(id)
										.map(|index| (*index).into())
										.ok_or(id.as_str())
								})
								.collect::<Result<_, _>>()?
						},
					)*
					styles,
				}
			};
		}

		let lookup = make_lookup!(nodes, blocks, binds, presets);

		self.geo_map.rebase(&lookup);
		self.maps.rebase(&lookup);

		Ok(())
	}
}

struct Lookup {
	nodes: Vec<Ref<Node>>,
	blocks: Vec<Ref<Block>>,
	binds: Vec<Ref<Bind>>,
	presets: Vec<Ref<Preset>>,
	styles: usize,
}

trait Rebase {
	fn rebase(&mut self, lookup: &Lookup);
}

macro_rules! impl_rebase_ref {
	($t:ty, $field:ident) => {
		impl Rebase for Ref<$t> {
			fn rebase(&mut self, lookup: &Lookup) {
				*self = lookup.$field[self.0];
			}
		}
	};
}

impl_rebase_ref!(Node, nodes);
impl_rebase_ref!(Block, blocks);
impl_rebase_ref!(Bind, binds);
impl_rebase_ref!(Preset, presets);

impl Rebase for Ref<Style> {
	fn rebase(&mut self, lookup: &Lookup) {
		self.0 += lookup.styles;
	}
}

impl<T: Rebase> Rebase for Option<T> {
	fn rebase(&mut self, lookup: &Lookup) {
		self.as_mut().map(|inner| inner.rebase(lookup));
	}
}

impl<T: Rebase> Rebase for Vec<T> {
	fn rebase(&mut self, lookup: &Lookup) {
		self.iter_mut().for_each(|inner| inner.rebase(lookup));
	}
}

impl Rebase for GeoMap {
	fn rebase(&mut self, lookup: &Lookup) {
		self.paths.rebase(lookup);
		self.targets.rebase(lookup);
		self.widgets.rebase(lookup);
	}
}

impl Rebase for Map {
	fn rebase(&mut self, lookup: &Lookup) {
		self.paths.rebase(lookup);
		self.targets.rebase(lookup);
		self.widgets.rebase(lookup);
	}
}

impl<T: Projectable> Rebase for Path<T> {
	fn rebase(&mut self, lookup: &Lookup) {
		self.display.rebase(lookup);
	}
}

impl Rebase for PathDisplay {
	fn rebase(&mut self, lookup: &Lookup) {
		match self {
			Self::Fixed { style } => style.rebase(lookup),
			Self::Node {
				node,
				off,
				on,
				selected,
			} => {
				node.rebase(lookup);
				off.rebase(lookup);
				on.rebase(lookup);
				selected.rebase(lookup);
			},
			Self::Bind {
				bind,
				off,
				on,
				pending,
			} => {
				bind.rebase(lookup);
				off.rebase(lookup);
				on.rebase(lookup);
				pending.rebase(lookup);
			},
		}
	}
}

impl<T: Projectable> Rebase for Target<T> {
	fn rebase(&mut self, lookup: &Lookup) {
		self.command.rebase(lookup);
	}
}

impl Rebase for TargetCommand {
	fn rebase(&mut self, lookup: &Lookup) {
		match self {
			Self::Node(node) => node.rebase(lookup),
			Self::Block(block) => block.rebase(lookup),
			Self::Preset(preset) => preset.rebase(lookup),
		}
	}
}

impl<T: Projectable> Rebase for Widget<T> {
	fn rebase(&mut self, lookup: &Lookup) {
		match self {
			Self::Countdown { target, .. } => target.rebase(lookup),
		}
	}
}

impl Rebase for ResetTarget {
	fn rebase(&mut self, lookup: &Lookup) {
		match self {
			Self::Node(node) => node.rebase(lookup),
			Self::Block(block) => block.rebase(lookup),
		}
	}
}
