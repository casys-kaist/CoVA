import numpy as np

MSEC_TO_NSEC = 1_000_000

def time_cv_to_gst(time_cv):
    return int(time_cv * MSEC_TO_NSEC)

def time_gst_to_cv(time_gst):
    return time_gst / MSEC_TO_NSEC

def arange_ts(start, end):
    TIMESTEP = 33_333_333
    TIMESTEP_3 = 100_000_000

    tmp = np.arange(start, end, TIMESTEP_3)
    ret = np.empty((tmp.size * 3, ), dtype=tmp.dtype)
    ret[0::3] = tmp
    ret[1::3] = tmp + TIMESTEP
    ret[2::3] = tmp + TIMESTEP * 2

    return ret
