use crate::consts::{angle_lower_bound, angle_upper_bound};

#[inline]
pub(crate) fn are_angles_opposite(a: i32, b: i32) -> bool {
    if b > 0 {
        if a == b - 180 {
            return true;
        }
    } else {
        if a == b + 180 {
            return true;
        }
    }
    false
}

#[inline]
pub(crate) fn rounded(x: f32) -> i32 {
    x.round() as i32
}

#[inline]
pub(crate) fn rad_to_deg(rad: f32) -> f32 {
    180.0 / std::f32::consts::PI * rad
}

#[inline]
pub(crate) fn atan2_round_degree(dx: i32, dy: i32) -> i32 {
    if dx == 0 {
        90
    } else {
        rounded(rad_to_deg(f32::atan2(dy as f32, dx as f32)))
    }
}

#[inline]
pub(crate) fn normalize_angle(deg: i32) -> i32 {
    if deg > 180 {
        deg - 360
    } else if deg <= -180 {
        deg + 360
    } else {
        deg
    }
}

#[inline]
pub(crate) fn average_angles(a: i32, b: i32) -> i32 {
    let mut avg = Averager::new();
    avg.push(a);
    avg.push(b);
    avg.average()
}

pub(crate) fn calculate_slope_in_degrees(dx: i32, dy: i32) -> i32 {
    if dx != 0 {
        let mut fi = rad_to_deg((dy as f32 / dx as f32).atan());
        if fi < 0.0 {
            if dx < 0 {
                fi += 180.0;
            }
        } else {
            if dx < 0 {
                fi -= 180.0;
            }
        }

        let fi = rounded(fi);
        if fi <= -180 {
            fi + 360
        } else {
            fi
        }
    } else {
        if dy <= 0 {
            -90
        } else {
            90
        }
    }
}

pub(crate) struct Averager {
    sum_of_negative: i32,
    number_of_negative: usize,
    sum_of_positive: i32,
    number_of_positive: usize,
}

impl Averager {
    #[inline]
    pub(crate) fn new() -> Self {
        Averager {
            sum_of_negative: 0,
            number_of_negative: 0,
            sum_of_positive: 0,
            number_of_positive: 0,
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, value: i32) {
        if value < 0 {
            self.sum_of_negative += value;
            self.number_of_negative += 1;
        } else {
            self.sum_of_positive += value;
            self.number_of_positive += 1;
        }
    }

    #[inline]
    pub(crate) fn average(self) -> i32 {
        let number_of_negative = self.number_of_negative.max(1);
        let number_of_positive = self.number_of_positive.max(1);
        let number_of_all = self.number_of_positive + self.number_of_negative;

        let mut fi = self.sum_of_positive as f32 / number_of_positive as f32
            - self.sum_of_negative as f32 / number_of_negative as f32;
        if fi > 180.0 {
            fi = (self.sum_of_positive
                + self.sum_of_negative
                + self.number_of_negative as i32 * 360) as f32
                / number_of_all as f32;
            if fi > 180.0 {
                fi -= 360.0;
            }
        } else {
            fi = (self.sum_of_positive + self.sum_of_negative) as f32 / number_of_all as f32;
        }

        let mut average = rounded(fi);
        if average <= -180 {
            average += 360
        }

        assert!(average > -180 && average <= 180);

        average
    }
}

#[inline]
pub(crate) fn are_angles_equal_with_tolerance(a: i32, b: i32) -> bool {
    let difference = (a - b).abs();
    return !(difference > angle_lower_bound() && difference < angle_upper_bound());
}
