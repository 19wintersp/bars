#include "radar_screen.hpp"

RadarScreen::RadarScreen(PtrDyn handler_) : handler(handler_) {}

void RadarScreen::SafeConvertCoordFromPositionToPixel(ES::CPosition a, POINT *out) {
	*out = ConvertCoordFromPositionToPixel(a);
}
