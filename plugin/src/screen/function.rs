#[derive(Clone, Copy)]
#[repr(u16)]
pub enum TagFunctionType {
	OpenAerodromeInput,
	SetAerodrome,
	OpenProfileList,
	SetProfile,
	OpenPresetList,
	ApplyPreset,
	OpenViewList,
	SetView,
	OpenHighlightList,
	SetHighlight,
	Max,
}

pub struct TagFunction {
	pub function: TagFunctionType,
	pub index: usize,
}

impl TagFunction {
	pub fn new(function: TagFunctionType, index: usize) -> Self {
		Self { function, index }
	}
}

impl TryFrom<i32> for TagFunction {
	type Error = ();

	fn try_from(from: i32) -> Result<Self, Self::Error> {
		let from = from as u32;

		let function = (from >> 16) as u16;
		if function >= TagFunctionType::Max as u16 {
			return Err(())
		}

		let index = from as usize & u16::MAX as usize;
		Ok(Self {
			function: unsafe { std::mem::transmute(function) },
			index,
		})
	}
}

impl From<TagFunction> for i32 {
	fn from(from: TagFunction) -> i32 {
		let function = (from.function as u16 as usize) << 16;
		(function | from.index) as u32 as i32
	}
}
