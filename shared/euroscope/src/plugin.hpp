#pragma once

#include "radar_screen.hpp"
#include "shared.hpp"

class Plugin : public ES::CPlugIn {
private:
	PtrDyn handler;

public:
	Plugin(
		const char *name, const char *version, const char *author,
		const char *copyright, PtrDyn handler
	);

	void SafeRadarTargetSelect(const char *, ES::CRadarTarget *) const;
	void SafeRadarTargetSelectFirst(ES::CRadarTarget *) const;
	void SafeRadarTargetSelectNext(ES::CRadarTarget, ES::CRadarTarget *) const;
	void SafeRadarTargetSelectASEL(ES::CRadarTarget *) const;

	void SafeFlightPlanSelect(const char *, ES::CFlightPlan *) const;
	void SafeFlightPlanSelectFirst(ES::CFlightPlan *) const;
	void SafeFlightPlanSelectNext(ES::CFlightPlan, ES::CFlightPlan *) const;
	void SafeFlightPlanSelectASEL(ES::CFlightPlan *) const;

	bool OnCompileCommand(const char *) override;
	void OnGetTagItem(
		ES::CFlightPlan, ES::CRadarTarget, int, int, char[16], int *, COLORREF *,
		double *
	) override;
	void OnFunctionCall(int, const char *, POINT, RECT) override;
	void OnTimer(int) override;
	RadarScreen *
	OnRadarScreenCreated(const char *, bool, bool, bool, bool) override;
};
