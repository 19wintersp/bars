#pragma once

#include "shared.hpp"

class RadarScreen : public ES::CRadarScreen {
private:
	PtrDyn handler;

public:
	RadarScreen(PtrDyn handler);

	void SafeConvertCoordFromPositionToPixel(ES::CPosition, POINT *);

	void OnAsrContentLoaded(bool) override;
	void OnRefresh(HDC, int) override;
	void OnAsrContentToBeClosed(void) override;
	bool OnCompileCommand(const char *) override;
	void OnOverScreenObject(int, const char *, POINT, RECT) override;
	void OnButtonDownScreenObject(int, const char *, POINT, RECT, int) override;
	void OnButtonUpScreenObject(int, const char *, POINT, RECT, int) override;
	void OnClickScreenObject(int, const char *, POINT, RECT, int) override;
	void OnDoubleClickScreenObject(int, const char *, POINT, RECT, int) override;
	void OnMoveScreenObject(int, const char *, POINT, RECT, bool) override;
	void OnFunctionCall(int, const char *, POINT, RECT) override;
};
