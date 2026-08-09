#include "safe.hpp"

namespace safe {
	ES::CPosition RadarTarget_GetPosition(const ES::CRadarTarget *rt) {
		return rt->GetPosition().GetPosition();
	}
}
