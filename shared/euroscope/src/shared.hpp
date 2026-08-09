#pragma once

#include <stdint.h>

#ifndef NULL
#define NULL ((void *) 0)
#endif

typedef struct tagPOINT {
	int32_t x, y;
} POINT;
typedef struct tagRECT {
	int32_t left, top, right, bottom;
} RECT;
typedef void *HDC;
typedef uint32_t COLORREF;

#include <EuroScopePlugIn.hpp>

namespace ES = EuroScopePlugIn;

typedef struct {
	void *data, *meta;
} PtrDyn;
