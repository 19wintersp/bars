mod button;
mod function;
mod highlight;
mod lookup;
mod renderer;
mod transform;

use self::button::Button;
use self::function::{TagFunction, TagFunctionType};
use self::highlight::Highlight;
use self::renderer::Renderer;
use crate::THEME_COLOR;
use crate::context::{Aerodrome, AerodromeMut, Context};

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::rc::Rc;

use bars_config::{FillStyle, Preset, Profile, Ref};
use bars_euroscope::{
	Area, Hdc, MouseEvent, Point, PopupListElementCheckbox, PopupListItemOptions,
	RadarScreen, RadarScreenHandler, RefreshPhase, ScreenObject, SettingStore,
};
use bars_graphics::{Brush, Font, FontFamily, Graphics};

use tracing::debug;

static FONT_FAMILY: &str = "EuroScope";
const FONT_SIZE: f32 = 12.0;

static SETTING_KEY_AERODROME: &str = "aerodrome";
static SETTING_KEY_HIGHLIGHT: &str = "highlight";

#[repr(i32)]
enum ObjectGroup {
	MapGeo = 0, // zero to ensure correct behaviour with ES
	MapView,
	Button,
	Max,
}

struct ContextWrapper {
	context: Rc<RefCell<Context>>,
	aerodrome: Option<String>,
}

impl ContextWrapper {
	fn set_aerodrome(&mut self, aerodrome: Option<String>) {
		if aerodrome != self.aerodrome {
			let mut context = self.context.borrow_mut();

			if let Some(current) = std::mem::replace(&mut self.aerodrome, aerodrome) {
				context.subscribe(&current, false);
			}

			if let Some(new) = &self.aerodrome {
				context.subscribe(new, true);
			}
		}
	}

	fn with_aerodrome(&self, f: impl FnOnce(&Aerodrome)) {
		let context = self.context.borrow();
		self
			.aerodrome
			.as_ref()
			.and_then(|aerodrome| context.aerodrome(aerodrome))
			.map(f);
	}

	fn with_aerodrome_mut(&mut self, f: impl FnOnce(AerodromeMut<'_>)) {
		let mut context = self.context.borrow_mut();
		self
			.aerodrome
			.as_ref()
			.and_then(|aerodrome| context.aerodrome_mut(aerodrome))
			.map(f);
	}
}

impl Drop for ContextWrapper {
	fn drop(&mut self) {
		if let Some(aerodrome) = &self.aerodrome {
			self.context.borrow_mut().subscribe(aerodrome, false);
		}
	}
}

pub struct Screen {
	context: ContextWrapper,
	geo: bool,
	graphics: GraphicsContext,
	button: Button,
	renderer: Renderer,
	highlight: Highlight,
	aerodrome_loaded: bool,
	profile_loaded: usize,
}

impl Screen {
	pub fn new(context: Rc<RefCell<Context>>, geo: bool) -> Self {
		Self {
			context: ContextWrapper {
				context,
				aerodrome: None,
			},
			geo,
			graphics: GraphicsContext {
				graphics: Graphics::new(),
				font: Font::new(&FontFamily::new(FONT_FAMILY), FONT_SIZE),
				brush: Brush::new(FillStyle::Fill, THEME_COLOR).unwrap(),
			},
			button: Button::new(),
			renderer: Renderer::new(geo),
			highlight: Highlight::None,
			aerodrome_loaded: false,
			profile_loaded: usize::MAX,
		}
	}

	fn set_aerodrome(
		&mut self,
		aerodrome: Option<String>,
		ctx: &mut RadarScreen,
	) {
		self.context.set_aerodrome(aerodrome);
		self.button.set_aerodrome(&self.context.aerodrome);

		ctx.save_setting(
			SETTING_KEY_AERODROME,
			self
				.context
				.aerodrome
				.as_ref()
				.map(|s| s.as_str())
				.unwrap_or(""),
			c"Active aerodrome",
		);
		ctx.request_backdrop_refresh();

		self.renderer = Renderer::new(self.geo);
		self.aerodrome_loaded = false;
		self.profile_loaded = usize::MAX;
	}

	fn set_profile(&mut self, profile: Ref<Profile>) {
		self
			.context
			.with_aerodrome_mut(|mut aerodrome| aerodrome.set_profile(profile));
	}

	fn apply_preset(&mut self, preset: Ref<Preset>) {
		self
			.context
			.with_aerodrome_mut(|mut aerodrome| aerodrome.apply_preset(preset));
	}

	fn open_menu(&self, ctx: &mut RadarScreen, area: Area) {
		let ctx = ctx.plugin();
		ctx.open_popup_list(c"BARS menu", true, area);

		self.context.with_aerodrome(|aerodrome| {
			for (key, value, function, disabled) in [
				(
					c"Aerodrome",
					self.context.aerodrome.as_ref().unwrap().as_str(),
					TagFunctionType::OpenAerodromeInput,
					false,
				),
				(
					c"Profile",
					aerodrome.profile_config().name.as_str(),
					TagFunctionType::OpenProfileList,
					false,
				),
				(
					c"Preset",
					"",
					TagFunctionType::OpenPresetList,
					aerodrome.config().config.presets.is_empty(),
				),
				if self.geo {
					(
						c"Highlight",
						self.highlight.name(),
						TagFunctionType::OpenHighlightList,
						false,
					)
				} else {
					(
						c"View",
						aerodrome
							.config()
							.maps
							.iter()
							.flat_map(|map| map.views.iter())
							.nth(self.renderer.view().unwrap().0)
							.map(|view| view.name.as_str())
							.unwrap_or(""),
						TagFunctionType::OpenViewList,
						false,
					)
				},
			] {
				let value = CString::new(value).unwrap();
				ctx.add_popup_list_item(
					key,
					Some(&value),
					TagFunction::new(function, 0).into(),
					PopupListItemOptions {
						disabled,
						..Default::default()
					},
				);
			}
		});
	}

	fn open_list<'a>(
		&self,
		ctx: &mut RadarScreen,
		area: Area,
		title: &CStr,
		function: TagFunctionType,
		selected: Option<usize>,
		names: impl Iterator<Item = &'a str>,
	) {
		let ctx = ctx.plugin();
		ctx.open_popup_list(title, false, area);

		for (i, name) in names.enumerate() {
			let name = CString::new(name).unwrap();
			ctx.add_popup_list_item(
				&name,
				None,
				TagFunction::new(function, i).into(),
				PopupListItemOptions {
					checkbox: match selected {
						None => PopupListElementCheckbox::NoCheckbox,
						Some(j) if i == j => PopupListElementCheckbox::Checked,
						Some(_) => PopupListElementCheckbox::Unchecked,
					},
					..Default::default()
				},
			);
		}
	}

	fn open_aerodrome_input(&self, ctx: &mut RadarScreen, area: Area) {
		let value = CString::new(
			self
				.context
				.aerodrome
				.as_ref()
				.map(|s| s.as_str())
				.unwrap_or(""),
		)
		.unwrap();
		ctx.plugin().open_popup_input(
			&value,
			TagFunction::new(TagFunctionType::SetAerodrome, 0).into(),
			area,
		);
	}

	fn render_highlights(&self, ctx: &mut RadarScreen) {
		let context = self.context.context.borrow();
		let positions = ctx
			.plugin()
			.radar_targets()
			.filter(|rt| {
				rt.callsign()
					.to_str()
					.is_ok_and(|callsign| context.is_pilot_connected(callsign))
			})
			.map(|rt| rt.position())
			.collect::<Vec<_>>();
		for position in positions {
			let p = ctx.project(position);
			self.highlight.render(p, &self.graphics);
		}
	}
}

impl RadarScreenHandler for Screen {
	fn init(&mut self, ctx: &mut RadarScreen) {
		self.set_aerodrome(ctx.load_setting::<str>(SETTING_KEY_AERODROME), ctx);
		self.highlight = ctx
			.load_setting::<Highlight>(SETTING_KEY_HIGHLIGHT)
			.unwrap_or_default();

		self.button.init(ctx);
		self.renderer.init(ctx);
	}

	fn refresh(&mut self, ctx: &mut RadarScreen, hdc: Hdc, phase: RefreshPhase) {
		self.graphics.graphics.set_hdc(hdc);

		match phase {
			RefreshPhase::Backdrop => {
				debug!("backdrop refresh");

				#[cfg(debug_assertions)]
				let start = std::time::Instant::now();

				self.context.with_aerodrome(|aerodrome| {
					self
						.renderer
						.render_backdrop(ctx, aerodrome, &self.graphics)
				});

				#[cfg(debug_assertions)]
				debug!("refresh took {:?}", start.elapsed());
			},
			RefreshPhase::BeforeTags => {
				let area = ctx.radar_area();
				ctx.add_screen_object(
					ScreenObject {
						group: if self.geo {
							ObjectGroup::MapGeo
						} else {
							ObjectGroup::MapView
						} as i32,
						id: c"",
						area,
					},
					false,
					c"Ensure BARS is first in the plugin list",
				);

				self.context.with_aerodrome(|aerodrome| {
					if !self.aerodrome_loaded {
						self.aerodrome_loaded = true;
						ctx.request_backdrop_refresh();
					} else if self.profile_loaded != aerodrome.profile().0 {
						self.profile_loaded = aerodrome.profile().0;
						ctx.request_backdrop_refresh();
					}

					self.renderer.render(ctx, aerodrome, &self.graphics)
				});
			},
			RefreshPhase::AfterTags => {
				if self.context.aerodrome.is_some() {
					self.render_highlights(ctx);
				}
			},
			RefreshPhase::AfterLists => {
				self.button.render(ctx, &self.graphics);

				ctx.add_screen_object(
					ScreenObject {
						group: ObjectGroup::Button as i32,
						id: c"",
						area: self.button.area(),
					},
					true,
					c"BARS menu",
				);
			},
		}
	}

	fn mouse_event(
		&mut self,
		ctx: &mut RadarScreen,
		event: MouseEvent,
		position: Point,
		object: ScreenObject,
	) {
		if object.group >= ObjectGroup::Max as i32 {
			return
		}

		match unsafe { std::mem::transmute(object.group) } {
			ObjectGroup::Max => unreachable!(),
			ObjectGroup::MapGeo | ObjectGroup::MapView => {
				let event = match (object.group, event) {
					(0, MouseEvent::Down(button)) => MouseEvent::Click(button),
					(_, event) => event,
				};

				if let MouseEvent::Click(_) = event {
					self.context.with_aerodrome_mut(|aerodrome| {
						self.renderer.mouse_event(ctx, aerodrome, event, position);
					});
				}
			},
			ObjectGroup::Button => match event {
				MouseEvent::Click(_) => {
					let area = self.button.area();
					if self.context.aerodrome.is_some() {
						self.open_menu(ctx, area);
					} else {
						self.open_aerodrome_input(ctx, area);
					}
				},
				MouseEvent::Drag | MouseEvent::DragEnd => {
					self.button.mouse_event(ctx, event, position);
				},
				_ => (),
			},
		}
	}

	fn call_tag_function(
		&mut self,
		ctx: &mut RadarScreen,
		code: i32,
		string: Option<&CStr>,
		_point: Point,
		area: Area,
	) {
		let Ok(TagFunction { function, index }) = code.try_into() else {
			return
		};

		debug!("tag function {code} called");

		match function {
			TagFunctionType::Max => unreachable!(),
			TagFunctionType::OpenAerodromeInput => {
				self.open_aerodrome_input(ctx, area);
			},
			TagFunctionType::OpenProfileList => {
				self.context.with_aerodrome(|aerodrome| {
					self.open_list(
						ctx,
						area,
						c"Set profile",
						TagFunctionType::SetProfile,
						Some(aerodrome.profile().0),
						aerodrome
							.config()
							.config
							.profiles
							.iter()
							.map(|profile| profile.name.as_str()),
					);
				})
			},
			TagFunctionType::OpenPresetList => {
				self.context.with_aerodrome(|aerodrome| {
					self.open_list(
						ctx,
						area,
						c"Apply preset",
						TagFunctionType::ApplyPreset,
						None,
						aerodrome
							.config()
							.config
							.presets
							.iter()
							.filter_map(|preset| preset.name.as_ref())
							.map(|name| name.as_str()),
					);
				})
			},
			TagFunctionType::OpenViewList => {
				self.context.with_aerodrome(|aerodrome| {
					self.open_list(
						ctx,
						area,
						c"Set view",
						TagFunctionType::SetView,
						Some(self.renderer.view().map(|r| r.0).unwrap_or_default()),
						aerodrome
							.config()
							.maps
							.iter()
							.flat_map(|map| map.views.iter())
							.map(|view| view.name.as_str()),
					);
				})
			},
			TagFunctionType::OpenHighlightList => self.open_list(
				ctx,
				area,
				c"Set highlight",
				TagFunctionType::SetHighlight,
				Some(self.highlight.into()),
				Highlight::ALL.iter().map(|highlight| highlight.name()),
			),
			TagFunctionType::SetAerodrome => {
				let aerodrome = string
					.map(|s| s.to_string_lossy().into_owned())
					.unwrap_or_default()
					.to_ascii_uppercase();
				match aerodrome.len() {
					0 => {
						self.set_aerodrome(None, ctx);
					},
					4 if aerodrome.chars().all(|c| c.is_ascii_uppercase()) => {
						self.set_aerodrome(Some(aerodrome), ctx);
					},
					_ => (),
				}
			},
			TagFunctionType::SetProfile => self.set_profile(index.into()),
			TagFunctionType::ApplyPreset => self.apply_preset(index.into()),
			TagFunctionType::SetView => {
				self.renderer.set_view(index.into());
				ctx.request_backdrop_refresh();
			},
			TagFunctionType::SetHighlight => {
				if let Ok(highlight) = index.try_into() {
					self.highlight = highlight;
					ctx.save_setting(
						SETTING_KEY_HIGHLIGHT,
						&highlight,
						c"Client aircraft highlight style",
					);
				}
			},
		}
	}
}

struct GraphicsContext {
	graphics: Graphics,
	font: Font,
	brush: Brush,
}
