#include "plugin.hpp"

Plugin::Plugin(
	const char *name, const char *version, const char *author,
	const char *copyright, PtrDyn handler_
)
	: CPlugIn(ES::COMPATIBILITY_CODE, name, version, author, copyright),
		handler(handler_) {}

void Plugin::SafeRadarTargetSelect(const char *a, ES::CRadarTarget *out) const {
	*out = RadarTargetSelect(a);
}
void Plugin::SafeRadarTargetSelectFirst(ES::CRadarTarget *out) const {
	*out = RadarTargetSelectFirst();
}
void Plugin::SafeRadarTargetSelectNext(ES::CRadarTarget a, ES::CRadarTarget *out) const {
	*out = RadarTargetSelectNext(a);
}
void Plugin::SafeRadarTargetSelectASEL(ES::CRadarTarget *out) const {
	*out = RadarTargetSelectASEL();
}

void Plugin::SafeFlightPlanSelect(const char *a, ES::CFlightPlan *out) const {
	*out = FlightPlanSelect(a);
}
void Plugin::SafeFlightPlanSelectFirst(ES::CFlightPlan *out) const {
	*out = FlightPlanSelectFirst();
}
void Plugin::SafeFlightPlanSelectNext(ES::CFlightPlan a, ES::CFlightPlan *out) const {
	*out = FlightPlanSelectNext(a);
}
void Plugin::SafeFlightPlanSelectASEL(ES::CFlightPlan *out) const {
	*out = FlightPlanSelectASEL();
}
