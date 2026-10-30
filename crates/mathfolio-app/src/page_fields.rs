//! Cached save-date presentations. No clock or filesystem access while painting.
use mathfolio_document::canvas::DateSavedFormat;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct SavedDate {
    formats: [String; 7],
}

impl Default for SavedDate {
    fn default() -> Self {
        #[cfg(target_arch = "wasm32")]
        let now = UNIX_EPOCH + std::time::Duration::from_millis(js_sys::Date::now() as u64);
        #[cfg(not(target_arch = "wasm32"))]
        let now = SystemTime::now();
        Self::from_timestamp(now).unwrap_or_else(|| Self {
            formats: Default::default(),
        })
    }
}

impl SavedDate {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_path(path: &Path) -> Option<Self> {
        Self::from_timestamp(std::fs::metadata(path).ok()?.modified().ok()?)
    }
    pub(crate) fn from_timestamp(timestamp: SystemTime) -> Option<Self> {
        let seconds: i64 = match timestamp.duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs().try_into().ok()?,
            Err(error) => {
                -i64::try_from(error.duration().as_secs()).ok()?
                    - i64::from(error.duration().subsec_nanos() != 0)
            }
        };
        let (year, month, day, short, long, separator) = local_date(seconds)?;
        Some(Self::from_parts(year, month, day, short, long, &separator))
    }
    fn from_parts(
        year: i32,
        month: u32,
        day: u32,
        short: String,
        long: String,
        separator: &str,
    ) -> Self {
        Self {
            formats: [
                format!("{month:02}{separator}{day:02}{separator}{year:04}"),
                format!(
                    "{month:02}{separator}{day:02}{separator}{:02}",
                    year.rem_euclid(100)
                ),
                format!("{day:02}{separator}{month:02}{separator}{year:04}"),
                format!(
                    "{day:02}{separator}{month:02}{separator}{:02}",
                    year.rem_euclid(100)
                ),
                format!("{year:04}-{month:02}-{day:02}"),
                short,
                long,
            ],
        }
    }
    pub(crate) fn format(&self, format: DateSavedFormat) -> String {
        self.formats[match format {
            DateSavedFormat::MonthDayYear => 0,
            DateSavedFormat::MonthDayShortYear => 1,
            DateSavedFormat::DayMonthYear => 2,
            DateSavedFormat::DayMonthShortYear => 3,
            DateSavedFormat::YearMonthDay => 4,
            DateSavedFormat::SystemShort => 5,
            DateSavedFormat::SystemLong => 6,
        }]
        .clone()
    }
}

type LocalDate = (i32, u32, u32, String, String, String);

#[cfg(windows)]
fn local_date(seconds: i64) -> Option<LocalDate> {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct SysTime {
        year: u16,
        month: u16,
        weekday: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FileTimeToSystemTime(file: *const FileTime, output: *mut SysTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(
            zone: *const std::ffi::c_void,
            utc: *const SysTime,
            local: *mut SysTime,
        ) -> i32;
        fn GetDateFormatEx(
            locale: *const u16,
            flags: u32,
            date: *const SysTime,
            format: *const u16,
            output: *mut u16,
            count: i32,
            calendar: *const u16,
        ) -> i32;
        fn GetLocaleInfoEx(locale: *const u16, kind: u32, output: *mut u16, count: i32) -> i32;
    }
    let ticks = seconds
        .checked_add(11_644_473_600)?
        .checked_mul(10_000_000)?;
    let ticks = u64::try_from(ticks).ok()?;
    let file = FileTime {
        low: ticks as u32,
        high: (ticks >> 32) as u32,
    };
    let mut utc = SysTime::default();
    let mut local = SysTime::default();
    // All structures and buffers have the documented Win32 C layouts. A null
    // timezone/locale selects this user's configured local time and date format.
    if unsafe { FileTimeToSystemTime(&file, &mut utc) } == 0
        || unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) } == 0
    {
        return None;
    }
    let date = |flags| {
        let mut buffer = [0u16; 512];
        let count = unsafe {
            GetDateFormatEx(
                std::ptr::null(),
                flags,
                &local,
                std::ptr::null(),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
                std::ptr::null(),
            )
        };
        (count > 1).then(|| String::from_utf16_lossy(&buffer[..count as usize - 1]))
    };
    let mut separator = [0u16; 32];
    let count = unsafe {
        GetLocaleInfoEx(
            std::ptr::null(),
            0x1f,
            separator.as_mut_ptr(),
            separator.len() as i32,
        )
    };
    let separator = if count > 1 {
        String::from_utf16_lossy(&separator[..count as usize - 1])
    } else {
        "/".into()
    };
    Some((
        local.year.into(),
        local.month.into(),
        local.day.into(),
        date(1)?,
        date(2)?,
        separator,
    ))
}

#[cfg(unix)]
fn local_date(seconds: i64) -> Option<LocalDate> {
    use std::ffi::CStr;
    #[allow(
        clippy::useless_conversion,
        reason = "time_t can be narrower than i64 on other Unix targets"
    )]
    let seconds: libc::time_t = seconds.try_into().ok()?;
    let mut date = std::mem::MaybeUninit::<libc::tm>::uninit();
    // localtime_r writes caller-owned storage and does not share libc's static tm.
    if unsafe { libc::localtime_r(&seconds, date.as_mut_ptr()) }.is_null() {
        return None;
    }
    let date = unsafe { date.assume_init() };
    let locale = unsafe { libc::newlocale(libc::LC_TIME_MASK, c"".as_ptr(), std::ptr::null_mut()) };
    if locale.is_null() {
        return None;
    }
    let format = |pattern: &CStr| {
        let mut buffer = [0u8; 512];
        let count = unsafe {
            libc::strftime_l(
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                pattern.as_ptr(),
                &date,
                locale,
            )
        };
        String::from_utf8_lossy(&buffer[..count]).into_owned()
    };
    let short = format(c"%x");
    // POSIX exposes a locale short-date pattern but no universal long-date
    // setting. Keep localized names; this independent long-date order is not
    // claimed to reproduce Windows/.NET culture tables on Linux.
    let long = format(c"%A, %d %B %Y");
    unsafe {
        libc::freelocale(locale);
    }
    Some((
        date.tm_year + 1900,
        (date.tm_mon + 1).try_into().ok()?,
        date.tm_mday.try_into().ok()?,
        short,
        long,
        "/".into(),
    ))
}

#[cfg(target_arch = "wasm32")]
fn local_date(seconds: i64) -> Option<LocalDate> {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(seconds as f64 * 1000.));
    let (year, month, day) = (
        date.get_full_year() as i32,
        date.get_month() + 1,
        date.get_date(),
    );
    Some((
        year,
        month,
        day,
        format!("{year:04}-{month:02}-{day:02}"),
        date.to_date_string().into(),
        "/".into(),
    ))
}

#[cfg(not(any(windows, unix, target_arch = "wasm32")))]
fn local_date(_seconds: i64) -> Option<LocalDate> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_native_templates_use_saved_date_and_the_documented_separators() {
        let date =
            SavedDate::from_parts(2004, 2, 29, "local short".into(), "local long".into(), "/");
        for (kind, expected) in [
            (DateSavedFormat::MonthDayYear, "02/29/2004"),
            (DateSavedFormat::MonthDayShortYear, "02/29/04"),
            (DateSavedFormat::DayMonthYear, "29/02/2004"),
            (DateSavedFormat::DayMonthShortYear, "29/02/04"),
            (DateSavedFormat::YearMonthDay, "2004-02-29"),
            (DateSavedFormat::SystemShort, "local short"),
            (DateSavedFormat::SystemLong, "local long"),
        ] {
            assert_eq!(date.format(kind), expected);
        }
        let local = SavedDate::from_parts(2004, 2, 29, "short".into(), "long".into(), ".");
        assert_eq!(local.format(DateSavedFormat::DayMonthYear), "29.02.2004");
        assert_eq!(local.format(DateSavedFormat::YearMonthDay), "2004-02-29");
    }
    #[test]
    fn timestamp_uses_local_calendar_and_cached_system_formats() {
        let date =
            SavedDate::from_timestamp(UNIX_EPOCH + std::time::Duration::from_secs(951_825_600))
                .unwrap();
        assert!(
            ["2000-02-28", "2000-02-29", "2000-03-01"]
                .contains(&date.format(DateSavedFormat::YearMonthDay).as_str())
        );
        assert!(!date.format(DateSavedFormat::SystemShort).is_empty());
        assert!(!date.format(DateSavedFormat::SystemLong).is_empty());
    }
}
