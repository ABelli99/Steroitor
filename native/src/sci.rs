//! Chiamate dirette a Scintilla (senza passare dalla coda dei messaggi) e le costanti usate.
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;

pub const SCI_SETCODEPAGE: u32 = 2037;
pub const SCI_CLEARALL: u32 = 2004;
pub const SCI_APPENDTEXT: u32 = 2282;
pub const SCI_GETLENGTH: u32 = 2006;
pub const SCI_GETTEXT: u32 = 2182;
pub const SCI_EMPTYUNDOBUFFER: u32 = 2175;
pub const SCI_SETSAVEPOINT: u32 = 2014;
pub const SCI_GOTOPOS: u32 = 2025;
pub const SCI_CREATEDOCUMENT: u32 = 2375;
pub const SCI_SETDOCPOINTER: u32 = 2358;
pub const SCI_RELEASEDOCUMENT: u32 = 2377;
pub const SCI_SETEOLMODE: u32 = 2031;
pub const SCI_SETILEXER: u32 = 4033;
pub const SCI_SETKEYWORDS: u32 = 4005;
pub const SCI_SETPROPERTY: u32 = 4004;
pub const SCI_STYLESETFORE: u32 = 2051;
pub const SCI_STYLESETBACK: u32 = 2052;
pub const SCI_STYLESETBOLD: u32 = 2053;
pub const SCI_STYLESETITALIC: u32 = 2054;
pub const SCI_STYLESETFONT: u32 = 2056;
pub const SCI_STYLESETSIZEFRACTIONAL: u32 = 2061;
pub const SCI_STYLECLEARALL: u32 = 2050;
pub const SCI_SETMARGINTYPEN: u32 = 2240;
pub const SCI_SETMARGINWIDTHN: u32 = 2242;
pub const SCI_SETMARGINLEFT: u32 = 2155;
pub const SCI_TEXTWIDTH: u32 = 2276;
pub const SCI_SETCARETLINEVISIBLE: u32 = 2096;
pub const SCI_SETCARETLINEVISIBLEALWAYS: u32 = 2655;
pub const SCI_SETELEMENTCOLOUR: u32 = 2753;
pub const SCI_SETCARETWIDTH: u32 = 2188;
pub const SCI_GETCURRENTPOS: u32 = 2008;
pub const SCI_LINEFROMPOSITION: u32 = 2166;
pub const SCI_GETCOLUMN: u32 = 2129;
pub const SCI_SETTABWIDTH: u32 = 2036;
pub const SCI_SETINDENT: u32 = 2122;
pub const SCI_SETUSETABS: u32 = 2124;
pub const SCI_GRABFOCUS: u32 = 2400;
pub const SCI_SETSCROLLWIDTHTRACKING: u32 = 2516;
pub const SCI_SETSCROLLWIDTH: u32 = 2274;
pub const SCI_SETMULTIPLESELECTION: u32 = 2563;
pub const SCI_SETADDITIONALSELECTIONTYPING: u32 = 2565;
pub const SCI_SETMULTIPASTE: u32 = 2614;
pub const SCI_BRACEMATCH: u32 = 2353;
pub const SCI_BRACEHIGHLIGHT: u32 = 2351;
pub const SCI_BRACEBADLIGHT: u32 = 2352;
pub const SCI_GETCHARAT: u32 = 2007;
pub const SCI_SETMARGINBACKN: u32 = 2250;
pub const SCI_SETINDENTATIONGUIDES: u32 = 2132;
pub const SCI_SETMODEVENTMASK: u32 = 2359;
pub const SCI_GETANCHOR: u32 = 2009;
pub const SCI_SETSEL: u32 = 2160;
pub const SCI_GETFIRSTVISIBLELINE: u32 = 2152;
pub const SCI_SETFIRSTVISIBLELINE: u32 = 2613;
pub const SCI_SETSEARCHFLAGS: u32 = 2198;
pub const SCI_SETTARGETRANGE: u32 = 2686;
pub const SCI_SEARCHINTARGET: u32 = 2197;
pub const SCI_GETTARGETEND: u32 = 2193;
pub const SCI_REPLACETARGET: u32 = 2194;
pub const SCI_TARGETFROMSELECTION: u32 = 2287;
pub const SCI_INDICSETSTYLE: u32 = 2080;
pub const SCI_INDICSETFORE: u32 = 2082;
pub const SCI_INDICSETALPHA: u32 = 2523;
pub const SCI_INDICSETOUTLINEALPHA: u32 = 2558;
pub const SCI_INDICSETUNDER: u32 = 2510;
pub const SCI_SETINDICATORCURRENT: u32 = 2500;
pub const SCI_INDICATORFILLRANGE: u32 = 2504;
pub const SCI_INDICATORCLEARRANGE: u32 = 2505;
pub const SCI_SCROLLCARET: u32 = 2169;
pub const SCI_BEGINUNDOACTION: u32 = 2078;
pub const SCI_ENDUNDOACTION: u32 = 2079;
pub const SCI_GETSELTEXT: u32 = 2161;
pub const SCI_GETSELECTIONSTART: u32 = 2143;
pub const SCI_GETSELECTIONEND: u32 = 2145;
pub const SCI_SETWRAPMODE: u32 = 2268;
pub const SCI_CONVERTEOLS: u32 = 2029;
pub const SCI_SETEXTRAASCENT: u32 = 2525;
pub const SCI_SETEXTRADESCENT: u32 = 2527;
pub const SCFIND_MATCHCASE: usize = 0x4;
pub const INDIC_ROUNDBOX: isize = 7;
pub const SC_WRAP_WORD: usize = 1;
pub const SCI_MARKERDEFINE: u32 = 2040;
pub const SCI_MARKERSETBACK: u32 = 2042;
pub const SCI_MARKERADD: u32 = 2043;
pub const SCI_MARKERDELETEALL: u32 = 2045;
pub const SCI_SETMARGINMASKN: u32 = 2244;
pub const SC_MARGIN_SYMBOL: isize = 0;
pub const SC_MARK_LEFTRECT: isize = 27;
pub const SC_MOD_INSERTTEXT: isize = 0x1;
pub const SC_MOD_DELETETEXT: isize = 0x2;
pub const SCN_MODIFIED: u32 = 2008;

pub const SCN_SAVEPOINTREACHED: u32 = 2002;
pub const SCN_SAVEPOINTLEFT: u32 = 2003;
pub const SCN_UPDATEUI: u32 = 2007;

pub const STYLE_DEFAULT: usize = 32;
pub const STYLE_LINENUMBER: usize = 33;
pub const STYLE_BRACELIGHT: usize = 34;
pub const STYLE_BRACEBAD: usize = 35;
pub const STYLE_INDENTGUIDE: usize = 37;

pub const SC_MARGIN_NUMBER: isize = 1;
pub const SC_EOL_CRLF: usize = 0;
pub const SC_EOL_CR: usize = 1;
pub const SC_EOL_LF: usize = 2;
pub const SC_CP_UTF8: usize = 65001;
pub const SC_IV_LOOKBOTH: usize = 3;

pub const SC_ELEMENT_SELECTION_BACK: usize = 11;
pub const SC_ELEMENT_SELECTION_ADDITIONAL_BACK: usize = 13;
pub const SC_ELEMENT_SELECTION_INACTIVE_BACK: usize = 17;
pub const SC_ELEMENT_CARET: usize = 40;
pub const SC_ELEMENT_CARET_LINE_BACK: usize = 50;

const SCI_GETDIRECTFUNCTION: u32 = 2184;
const SCI_GETDIRECTPOINTER: u32 = 2185;

/// Struttura delle notifiche WM_NOTIFY di Scintilla; servono solo l'intestazione e la posizione.
#[repr(C)]
pub struct Notification {
    pub hwnd_from: HWND,
    pub id_from: usize,
    pub code: u32,
    pub position: isize,
}

type DirectFunction = unsafe extern "C" fn(isize, u32, usize, isize) -> isize;

pub struct Sci {
    pub hwnd: HWND,
    function: DirectFunction,
    pointer: isize,
}

impl Sci {
    pub fn new(hwnd: HWND) -> Self {
        // SAFETY: SCI_GETDIRECTFUNCTION restituisce il puntatore alla funzione di Scintilla con questa firma.
        unsafe {
            let function = SendMessageW(hwnd, SCI_GETDIRECTFUNCTION, 0, 0);
            let pointer = SendMessageW(hwnd, SCI_GETDIRECTPOINTER, 0, 0);
            Sci { hwnd, function: std::mem::transmute::<isize, DirectFunction>(function), pointer }
        }
    }

    pub fn call(&self, message: u32, wparam: usize, lparam: isize) -> isize {
        // SAFETY: puntatori ottenuti da questa stessa finestra Scintilla, che vive quanto l'app.
        unsafe { (self.function)(self.pointer, message, wparam, lparam) }
    }

    /// Per i messaggi che vogliono una stringa C (nomi di font, parole chiave, proprietà).
    pub fn call_str(&self, message: u32, wparam: usize, text: &str) -> isize {
        let text = std::ffi::CString::new(text).unwrap_or_default();
        self.call(message, wparam, text.as_ptr() as isize)
    }

    pub fn set_property(&self, key: &str, value: &str) {
        let key = std::ffi::CString::new(key).unwrap_or_default();
        let value = std::ffi::CString::new(value).unwrap_or_default();
        self.call(SCI_SETPROPERTY, key.as_ptr() as usize, value.as_ptr() as isize);
    }

    pub fn set_text(&self, text: &[u8]) {
        self.call(SCI_CLEARALL, 0, 0);
        self.call(SCI_APPENDTEXT, text.len(), text.as_ptr() as isize);
        self.call(SCI_EMPTYUNDOBUFFER, 0, 0);
        self.call(SCI_SETSAVEPOINT, 0, 0);
        self.call(SCI_GOTOPOS, 0, 0);
    }

    pub fn text(&self) -> String {
        let length = self.call(SCI_GETLENGTH, 0, 0) as usize;
        let mut buffer = vec![0u8; length + 1];
        self.call(SCI_GETTEXT, length + 1, buffer.as_mut_ptr() as isize);
        buffer.truncate(length);
        String::from_utf8_lossy(&buffer).into_owned()
    }

    pub fn style(&self, style: usize, fore: u32) {
        self.call(SCI_STYLESETFORE, style, fore as isize);
    }
}
