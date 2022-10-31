// use std::panic::Location;
// use std::collections::HashMap;
// use std::time::Duration;

// static mut STATS: Option<HashMap<usize, (u128, u64)>> = None;
// static mut REPORTS: u64 = 0;

#[track_caller]
#[inline]
pub fn timeit<T>(f: impl FnOnce() -> T) -> T {
    return f();
    // let a = std::time::Instant::now();
    // let result = f();
    // let b = a.elapsed();

    // unsafe {
    //     if STATS.is_none() {
    //         STATS = Some(HashMap::new());
    //     }
    //     REPORTS += 1;

    //     let stats = STATS.as_mut().unwrap();
    //     stats.entry(Location::caller() as *const _ as usize).and_modify(|v| {
    //         v.0 += b.as_nanos();
    //         v.1 += 1;
    //     }).or_insert((b.as_nanos(), 1));

    //     if REPORTS % 1000000 == 0 {
    //         let total_time: u128 = stats.values().map(|it| it.0).sum();

    //         eprintln!("Summary:");
    //         for (ptr, (time, _events)) in stats.iter() {
    //             eprintln!("{} {:?} {:.02}%",
    //                       std::mem::transmute::<_, &'static Location<'static>>(*ptr),
    //                       Duration::from_micros((*time / 1000) as u64),
    //                       (*time as f64) / (total_time as f64) * 100.0
    //             );
    //         }
    //     }
    // }

    // result
}
