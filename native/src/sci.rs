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
pub const SCI_ADDREFDOCUMENT: u32 = 2376;
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
pub const SCI_POSITIONFROMPOINTCLOSE: u32 = 2023;
pub const SCI_SETEMPTYSELECTION: u32 = 2556;
pub const INDIC_PLAIN: isize = 0;
pub const SCI_INDICSETALPHA: u32 = 2523;
pub const SCI_INDICSETOUTLINEALPHA: u32 = 2558;
pub const SCI_INDICSETUNDER: u32 = 2510;
pub const SCI_SETINDICATORCURRENT: u32 = 2500;
pub const SCI_INDICATORFILLRANGE: u32 = 2504;
pub const SCI_INDICATORCLEARRANGE: u32 = 2505;
pub const SCI_SCROLLCARET: u32 = 2169;
pub const SCI_BEGINUNDOACTION: u32 = 2078;
pub const SCI_ENDUNDOACTION: u32 = 2079;
pub const SCI_GETLINE: u32 = 2153;
pub const SCI_LINELENGTH: u32 = 2350;
pub const SCI_GETLINEINDENTATION: u32 = 2127;
pub const SCI_SETLINEINDENTATION: u32 = 2126;
pub const SCI_GETLINEINDENTPOSITION: u32 = 2128;
pub const SCI_GETEOLMODE: u32 = 2030;
pub const SCI_INSERTTEXT: u32 = 2003;
pub const SCI_GETINDENT: u32 = 2123;
pub const SCI_GETTABWIDTH: u32 = 2121;
pub const SCI_GETSELECTIONS: u32 = 2570;
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
pub const SCN_MARGINCLICK: u32 = 2010;
pub const SCI_SETMARGINSENSITIVEN: u32 = 2246;
pub const SC_MARGIN_TEXT: isize = 4;
pub const SCI_MARGINSETTEXT: u32 = 2530;
pub const SCI_MARGINSETSTYLE: u32 = 2532;
pub const SCI_MARGINTEXTCLEARALL: u32 = 2536;
pub const SCI_POSITIONFROMLINE: u32 = 2167;
pub const SCI_GETDOCPOINTER: u32 = 2357;
pub const SCI_AUTOCSHOW: u32 = 2100;
pub const SCI_AUTOCCANCEL: u32 = 2101;
pub const SCI_AUTOCACTIVE: u32 = 2102;
pub const SCI_AUTOCSETSEPARATOR: u32 = 2106;
pub const SCI_AUTOCSETIGNORECASE: u32 = 2115;
pub const SCI_AUTOCSETMAXHEIGHT: u32 = 2210;
pub const SCI_AUTOCSETORDER: u32 = 2660;
pub const SC_ORDER_CUSTOM: usize = 2;
pub const SCI_CALLTIPSHOW: u32 = 2200;
pub const SCI_CALLTIPCANCEL: u32 = 2201;
pub const SCI_CALLTIPSETBACK: u32 = 2205;
pub const SCI_CALLTIPSETFORE: u32 = 2206;
pub const SCI_SETMOUSEDWELLTIME: u32 = 2264;
pub const SCI_WORDSTARTPOSITION: u32 = 2266;
pub const SCI_WORDENDPOSITION: u32 = 2267;
pub const INDIC_SQUIGGLEPIXMAP: isize = 13;
pub const SC_ELEMENT_LIST: usize = 0;
pub const SC_ELEMENT_LIST_BACK: usize = 1;
pub const SC_ELEMENT_LIST_SELECTED: usize = 2;
pub const SC_ELEMENT_LIST_SELECTED_BACK: usize = 3;
pub const SCN_CHARADDED: u32 = 2001;
pub const SCN_DWELLSTART: u32 = 2016;
pub const SCN_DWELLEND: u32 = 2017;
pub const SCN_AUTOCCANCELLED: u32 = 2025;
pub const SCN_AUTOCCOMPLETED: u32 = 2030;
pub const SCN_AUTOCSELECTIONCHANGE: u32 = 2032;

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

/// Inizio della struttura delle notifiche WM_NOTIFY di Scintilla (SCNotification), fino ai campi usati.
#[repr(C)]
pub struct Notification {
    pub hwnd_from: HWND,
    pub id_from: usize,
    pub code: u32,
    pub position: isize,
    pub ch: i32,
    pub modifiers: i32,
    pub modification_type: i32,
    pub text: *const std::ffi::c_char,
    pub length: isize,
    pub lines_added: isize,
    pub message: i32,
    pub wparam: usize,
    pub lparam: isize,
    pub line: isize,
    pub fold_level_now: i32,
    pub fold_level_prev: i32,
    pub margin: i32,
}

impl Notification {
    /// Il testo della notifica (es. la voce scelta nella lista), se c'è.
    pub fn text(&self) -> String {
        if self.text.is_null() {
            return String::new();
        }
        // SAFETY: per le notifiche che lo valorizzano, Scintilla passa una stringa C valida per la durata della notifica.
        unsafe { std::ffi::CStr::from_ptr(self.text) }.to_string_lossy().into_owned()
    }
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

    /// Testo della riga, con il fine riga.
    pub fn line(&self, line: usize) -> String {
        let length = self.call(SCI_LINELENGTH, line, 0).max(0) as usize;
        let mut buffer = vec![0u8; length + 1];
        self.call(SCI_GETLINE, line, buffer.as_mut_ptr() as isize);
        buffer.truncate(length);
        String::from_utf8_lossy(&buffer).into_owned()
    }

    pub fn style(&self, style: usize, fore: u32) {
        self.call(SCI_STYLESETFORE, style, fore as isize);
    }
}
