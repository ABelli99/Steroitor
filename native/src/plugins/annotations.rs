//! Spring e Lombok nel plugin Java, senza server. Completamento: nomi e attributi delle annotazioni, proprietà di
//! `@Value("${…}")` dai file application. Controllo: quello che il compilatore non vede (proxy di Spring, path
//! variable, proprietà mancanti, iniezione nei campi) e le combinazioni di Lombok che non compilano o non fanno quello
//! che sembra. Le annotazioni contano solo se gli import le fanno risalire a Spring o a Lombok.
use std::collections::BTreeSet;
use std::ops::Range;
use std::path::Path;

use super::jvm_source::{is_ident, mask};
use super::protocol::Diagnostic;
use super::{position_of, Local};

type Table = &'static [(&'static str, &'static [(&'static str, &'static [&'static str])])];

const LOG: &[&str] = &["topic"];
const CONSTRUCTOR: &[&str] = &["access", "staticName", "onConstructor"];
const LOMBOK: Table = &[
    (
        "lombok",
        &[
            ("Data", &["staticConstructor"]),
            ("Value", &["staticConstructor"]),
            ("Builder", &["builderMethodName", "buildMethodName", "builderClassName", "toBuilder", "access", "setterPrefix"]),
            ("Getter", &["value", "onMethod", "lazy"]),
            ("Setter", &["value", "onMethod", "onParam"]),
            ("With", &["value", "onMethod", "onParam"]),
            ("ToString", &["callSuper", "includeFieldNames", "exclude", "of", "onlyExplicitlyIncluded", "doNotUseGetters"]),
            ("EqualsAndHashCode", &["callSuper", "exclude", "of", "onlyExplicitlyIncluded", "doNotUseGetters", "cacheStrategy"]),
            ("NoArgsConstructor", &["force", "access", "staticName", "onConstructor"]),
            ("AllArgsConstructor", CONSTRUCTOR),
            ("RequiredArgsConstructor", CONSTRUCTOR),
            ("NonNull", &[]),
            ("Singular", &["value", "ignoreNullCollections"]),
            ("SneakyThrows", &["value"]),
            ("Cleanup", &["value"]),
            ("Synchronized", &["value"]),
            ("CustomLog", LOG),
        ],
    ),
    ("lombok.extern.slf4j", &[("Slf4j", LOG), ("XSlf4j", LOG)]),
    ("lombok.extern.log4j", &[("Log4j", LOG), ("Log4j2", LOG)]),
    ("lombok.extern.java", &[("Log", LOG)]),
    ("lombok.extern.apachecommons", &[("CommonsLog", LOG)]),
    ("lombok.extern.jbosslog", &[("JBossLog", LOG)]),
    ("lombok.extern.flogger", &[("Flogger", &[])]),
];
const LOGGERS: [&str; 9] = ["Slf4j", "XSlf4j", "Log4j", "Log4j2", "Log", "CommonsLog", "JBossLog", "Flogger", "CustomLog"];

const MAPPING: &[&str] = &["value", "path", "params", "headers", "consumes", "produces", "name"];
const BINDING: &[&str] = &["value", "name", "required", "defaultValue"];
const ADVICE: &[&str] = &["value", "basePackages", "basePackageClasses", "assignableTypes", "annotations"];
const CACHE: &[&str] = &["value", "cacheNames", "key", "keyGenerator", "cacheManager", "cacheResolver", "condition"];
const SPRING: Table = &[
    ("org.springframework.stereotype", &[("Component", &["value"]), ("Service", &["value"]), ("Repository", &["value"]), ("Controller", &["value"])]),
    (
        "org.springframework.web.bind.annotation",
        &[
            ("RestController", &["value"]),
            ("RequestMapping", &["value", "path", "method", "params", "headers", "consumes", "produces", "name"]),
            ("GetMapping", MAPPING),
            ("PostMapping", MAPPING),
            ("PutMapping", MAPPING),
            ("DeleteMapping", MAPPING),
            ("PatchMapping", MAPPING),
            ("PathVariable", &["value", "name", "required"]),
            ("RequestParam", BINDING),
            ("RequestHeader", BINDING),
            ("CookieValue", BINDING),
            ("RequestBody", &["required"]),
            ("ResponseBody", &[]),
            ("ResponseStatus", &["value", "code", "reason"]),
            ("ExceptionHandler", &["value"]),
            ("ControllerAdvice", ADVICE),
            ("RestControllerAdvice", ADVICE),
            ("CrossOrigin", &["value", "origins", "originPatterns", "allowedHeaders", "exposedHeaders", "methods", "allowCredentials", "maxAge"]),
            ("ModelAttribute", &["value", "name", "binding"]),
        ],
    ),
    ("org.springframework.beans.factory.annotation", &[("Autowired", &["required"]), ("Value", &["value"]), ("Qualifier", &["value"])]),
    (
        "org.springframework.context.annotation",
        &[
            ("Configuration", &["value", "proxyBeanMethods"]),
            ("Bean", &["value", "name", "initMethod", "destroyMethod", "autowireCandidate"]),
            ("ComponentScan", &["value", "basePackages", "basePackageClasses", "includeFilters", "excludeFilters", "lazyInit"]),
            ("Import", &["value"]),
            ("Profile", &["value"]),
            ("Primary", &[]),
            ("Lazy", &["value"]),
            ("Scope", &["value", "scopeName", "proxyMode"]),
            ("PropertySource", &["value", "name", "ignoreResourceNotFound", "encoding"]),
            ("DependsOn", &["value"]),
        ],
    ),
    (
        "org.springframework.transaction.annotation",
        &[
            ("Transactional", &["value", "transactionManager", "label", "propagation", "isolation", "timeout", "timeoutString", "readOnly", "rollbackFor", "rollbackForClassName", "noRollbackFor", "noRollbackForClassName"]),
            ("EnableTransactionManagement", &[]),
        ],
    ),
    (
        "org.springframework.scheduling.annotation",
        &[
            ("Async", &["value"]),
            ("Scheduled", &["cron", "zone", "fixedDelay", "fixedDelayString", "fixedRate", "fixedRateString", "initialDelay", "initialDelayString", "timeUnit"]),
            ("EnableAsync", &[]),
            ("EnableScheduling", &[]),
        ],
    ),
    ("org.springframework.cache.annotation", &[("Cacheable", CACHE), ("CacheEvict", CACHE), ("CachePut", CACHE), ("EnableCaching", &[])]),
    (
        "org.springframework.boot.autoconfigure",
        &[("SpringBootApplication", &["exclude", "excludeName", "scanBasePackages", "scanBasePackageClasses", "proxyBeanMethods"]), ("EnableAutoConfiguration", &["exclude", "excludeName"])],
    ),
    ("org.springframework.boot.context.properties", &[("ConfigurationProperties", &["value", "prefix", "ignoreInvalidFields", "ignoreUnknownFields"]), ("EnableConfigurationProperties", &["value"])]),
    (
        "org.springframework.boot.autoconfigure.condition",
        &[
            ("ConditionalOnProperty", &["value", "prefix", "name", "havingValue", "matchIfMissing"]),
            ("ConditionalOnMissingBean", &["value", "type", "name"]),
            ("ConditionalOnBean", &["value", "type", "name"]),
            ("ConditionalOnClass", &["value", "name"]),
        ],
    ),
];
const MAPPINGS: [&str; 6] = ["RequestMapping", "GetMapping", "PostMapping", "PutMapping", "DeleteMapping", "PatchMapping"];
const PROXIED: [&str; 5] = ["Transactional", "Async", "Cacheable", "CacheEvict", "CachePut"];
const STEREOTYPES: [&str; 4] = ["Component", "Service", "Controller", "RestController"];
const PRIMITIVES: [&str; 8] = ["boolean", "byte", "char", "short", "int", "long", "float", "double"];
const MODIFIERS: [&str; 13] = ["public", "protected", "private", "static", "final", "abstract", "transient", "volatile", "synchronized", "native", "default", "strictfp", "sealed"];

fn tables(spring: bool, lombok: bool) -> impl Iterator<Item = &'static (&'static str, &'static [(&'static str, &'static [&'static str])])> {
    let spring = if spring { SPRING } else { &[] };
    let lombok = if lombok { LOMBOK } else { &[] };
    spring.iter().chain(lombok)
}

struct Annotation {
    /// Come è scritta, anche qualificata (`lombok.Data`, `Builder.Default`).
    name: String,
    /// Dalla `@` alla fine del nome.
    at: Range<usize>,
    /// Dentro le parentesi.
    args: Option<Range<usize>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Type,
    Field,
    Method,
    Parameter,
}

struct Decl {
    annotations: Vec<Annotation>,
    modifiers: Vec<String>,
    kind: Kind,
    /// Parola chiave dei tipi (`class`, `interface`, `enum`, `record`, `@interface`), tipo di campi, metodi e parametri.
    detail: String,
    name: String,
    /// Campo con un valore iniziale; classe che estende un'altra.
    flag: bool,
    /// Membri dei tipi, parametri dei metodi.
    children: Vec<Decl>,
}

impl Decl {
    fn has(&self, modifier: &str) -> bool {
        self.modifiers.iter().any(|candidate| candidate == modifier)
    }

    fn members(&self, kind: Kind) -> impl Iterator<Item = &Decl> {
        self.children.iter().filter(move |child| child.kind == kind)
    }
}

/// Le dichiarazioni del file, abbastanza per le annotazioni: tipi (anche annidati), campi, metodi e parametri.
struct Parser<'a> {
    text: &'a str,
    m: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while self.m.get(self.pos).is_some_and(u8::is_ascii_whitespace) {
            self.pos += 1;
        }
    }

    fn ident_at(&self, at: usize) -> Range<usize> {
        let mut end = at;
        while self.m.get(end).copied().is_some_and(is_ident) {
            end += 1;
        }
        at..end
    }

    /// Dopo la parentesi che chiude quella aperta in `open`; la fine del testo se manca.
    fn close(&self, open: usize) -> usize {
        let mut depth = 0usize;
        for (index, byte) in self.m.iter().enumerate().skip(open) {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return index + 1;
                    }
                }
                _ => {}
            }
        }
        self.m.len()
    }

    /// Dopo i parametri di tipo `<…>`; si ferma prima di un carattere che non può starci.
    fn close_angle(&self, open: usize) -> usize {
        let mut depth = 0usize;
        for (index, byte) in self.m.iter().enumerate().skip(open) {
            match byte {
                b'<' => depth += 1,
                b'>' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return index + 1;
                    }
                }
                b';' | b'{' | b'(' | b')' | b'=' => return index,
                _ => {}
            }
        }
        self.m.len()
    }

    fn skip_statement(&mut self, end: usize) {
        while self.pos < end {
            match self.m[self.pos] {
                b';' => return self.pos += 1,
                b'(' | b'[' | b'{' => self.pos = self.close(self.pos),
                _ => self.pos += 1,
            }
        }
    }

    fn annotation(&mut self) -> Option<Annotation> {
        let start = self.pos;
        let mut end = start + 1;
        loop {
            let ident = self.ident_at(end);
            if ident.is_empty() {
                break;
            }
            end = ident.end;
            if self.m.get(end) != Some(&b'.') || !self.m.get(end + 1).copied().is_some_and(is_ident) {
                break;
            }
            end += 1;
        }
        let name = self.text.get(start + 1..end).filter(|name| !name.is_empty() && *name != "interface")?.to_string();
        self.pos = end;
        self.skip_ws();
        let args = if self.m.get(self.pos) == Some(&b'(') {
            let close = self.close(self.pos);
            let args = self.pos + 1..close.saturating_sub(1).max(self.pos + 1);
            self.pos = close;
            Some(args)
        } else {
            self.pos = end;
            None
        };
        Some(Annotation { name, at: start..end, args })
    }

    fn prefix(&mut self) -> (Vec<Annotation>, Vec<String>) {
        let (mut annotations, mut modifiers) = (Vec::new(), Vec::new());
        loop {
            self.skip_ws();
            if self.m.get(self.pos) == Some(&b'@') {
                match self.annotation() {
                    Some(annotation) => annotations.push(annotation),
                    None => break,
                }
                continue;
            }
            let word = self.ident_at(self.pos);
            if !MODIFIERS.contains(&&self.text[word.clone()]) {
                break;
            }
            modifiers.push(self.text[word.clone()].to_string());
            self.pos = word.end;
        }
        (annotations, modifiers)
    }

    fn members(&mut self, end: usize, constants: bool) -> Vec<Decl> {
        if constants {
            // Le costanti di un enum, fino al primo `;`.
            while self.pos < end && self.m[self.pos] != b';' {
                self.pos = if matches!(self.m[self.pos], b'(' | b'[' | b'{') { self.close(self.pos) } else { self.pos + 1 };
            }
        }
        let mut list = Vec::new();
        while self.pos < end {
            self.skip_ws();
            if self.pos >= end {
                break;
            }
            let before = self.pos;
            match self.m[self.pos] {
                b';' | b'}' | b')' => self.pos += 1,
                b'{' => self.pos = self.close(self.pos),
                _ => list.extend(self.declaration(end)),
            }
            if self.pos == before {
                self.pos += 1;
            }
        }
        list
    }

    fn declaration(&mut self, end: usize) -> Option<Decl> {
        let (annotations, modifiers) = self.prefix();
        if self.pos >= end {
            return None;
        }
        if self.m[self.pos] == b'{' {
            self.pos = self.close(self.pos);
            return None;
        }
        let word = if self.m[self.pos..].starts_with(b"@interface") { self.pos..self.pos + 10 } else { self.ident_at(self.pos) };
        let keyword = &self.text[word.clone()];
        if keyword == "package" || keyword == "import" {
            self.skip_statement(end);
            return None;
        }
        if ["class", "interface", "enum", "record", "@interface"].contains(&keyword) {
            self.pos = word.end;
            self.skip_ws();
            let at = self.ident_at(self.pos);
            if at.is_empty() {
                return None;
            }
            self.pos = at.end;
            let header = self.pos;
            while self.pos < end && !matches!(self.m[self.pos], b'{' | b';') {
                self.pos = if self.m[self.pos] == b'(' { self.close(self.pos) } else { self.pos + 1 };
            }
            let extends = keyword == "class" && self.text[header..self.pos].split(|ch: char| !ch.is_alphanumeric()).any(|word| word == "extends");
            let mut children = Vec::new();
            if self.m.get(self.pos) == Some(&b'{') {
                let close = self.close(self.pos);
                self.pos += 1;
                children = self.members(close.saturating_sub(1), keyword == "enum");
                self.pos = close;
            }
            let name = self.text[at.clone()].to_string();
            return Some(Decl { annotations, modifiers, kind: Kind::Type, detail: keyword.to_string(), name, flag: extends, children });
        }
        // Campo o metodo: il nome è l'ultimo identificatore prima di `(`, `=`, `;` o `,`.
        let start = self.pos;
        let mut stop = self.pos;
        let mut last: Option<Range<usize>> = None;
        while stop < end {
            let byte = self.m[stop];
            if is_ident(byte) {
                let ident = self.ident_at(stop);
                stop = ident.end;
                last = Some(ident);
                continue;
            }
            match byte {
                b'(' | b';' | b'=' | b',' | b'{' | b')' | b'}' => break,
                b'<' => stop = self.close_angle(stop),
                b'[' => stop = self.close(stop),
                b'@' => {
                    self.pos = stop;
                    stop = if self.annotation().is_some() { self.pos } else { stop + 1 };
                }
                _ => stop += 1,
            }
        }
        self.pos = stop;
        let at = last?;
        let detail = self.text[start..at.start].trim().to_string();
        let name = self.text[at.clone()].to_string();
        match self.m.get(stop) {
            Some(b'(') => {
                let close = self.close(stop);
                self.pos = stop + 1;
                let parameters = self.parameters(close.saturating_sub(1));
                self.pos = close;
                // Fino al corpo o al `;` (metodi astratti, `default` dei metodi delle annotazioni).
                while self.pos < end {
                    match self.m[self.pos] {
                        b'{' => {
                            self.pos = self.close(self.pos);
                            break;
                        }
                        b';' => {
                            self.pos += 1;
                            break;
                        }
                        b'(' => self.pos = self.close(self.pos),
                        _ => self.pos += 1,
                    }
                }
                Some(Decl { annotations, modifiers, kind: Kind::Method, detail, name, flag: false, children: parameters })
            }
            Some(b'=' | b';' | b',') => {
                let initialized = self.m[stop] == b'=';
                self.skip_statement(end);
                Some(Decl { annotations, modifiers, kind: Kind::Field, detail, name, flag: initialized, children: Vec::new() })
            }
            _ => None,
        }
    }

    fn parameters(&mut self, end: usize) -> Vec<Decl> {
        let mut list = Vec::new();
        while self.pos < end {
            let (annotations, modifiers) = self.prefix();
            let start = self.pos;
            let mut stop = self.pos;
            let mut last = None;
            while stop < end && self.m[stop] != b',' {
                if is_ident(self.m[stop]) {
                    let ident = self.ident_at(stop);
                    stop = ident.end;
                    last = Some(ident);
                    continue;
                }
                stop = match self.m[stop] {
                    b'<' => self.close_angle(stop),
                    b'(' | b'[' | b'{' => self.close(stop),
                    _ => stop + 1,
                };
            }
            if let Some(at) = last.filter(|at| at.start >= start) {
                let detail = self.text[start..at.start].trim().trim_end_matches("...").trim().to_string();
                let name = self.text[at.clone()].to_string();
                list.push(Decl { annotations, modifiers, kind: Kind::Parameter, detail, name, flag: false, children: Vec::new() });
            }
            self.pos = stop + 1;
        }
        list
    }
}

fn imports(masked: &str) -> Vec<String> {
    masked
        .split(';')
        .filter_map(|statement| statement.trim().strip_prefix("import"))
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map(|rest| rest.split_whitespace().collect::<String>())
        .filter(|import| !import.starts_with("static"))
        .collect()
}

struct Checker<'a> {
    text: &'a str,
    m: Vec<u8>,
    imports: Vec<String>,
    spring: bool,
    lombok: bool,
    list: Vec<Diagnostic>,
}

impl Checker<'_> {
    /// Pacchetto e nome dell'annotazione se è di Spring o di Lombok: qualificata, importata o da un import con `*`
    /// (vince l'import esplicito, come in Java).
    fn resolve(&self, written: &str) -> Option<(&'static str, &'static str)> {
        let (package, name) = written.rsplit_once('.').map_or((None, written), |(package, name)| (Some(package), name));
        let known = || tables(self.spring, self.lombok).flat_map(|(package, list)| list.iter().map(move |(name, _)| (*package, *name))).filter(move |(_, candidate)| *candidate == name);
        if let Some(package) = package {
            return known().find(|(candidate, _)| *candidate == package);
        }
        if let Some(import) = self.imports.iter().find(|import| import.rsplit('.').next() == Some(name)) {
            return known().find(|(package, _)| *import == format!("{package}.{name}"));
        }
        known().find(|(package, _)| self.imports.iter().any(|import| *import == format!("{package}.*")))
    }

    fn is(&self, annotation: &Annotation, framework: &str, name: &str) -> bool {
        self.resolve(&annotation.name).is_some_and(|(package, found)| package.starts_with(framework) && found == name)
    }

    fn lombok_on<'d>(&self, decl: &'d Decl, name: &str) -> Option<&'d Annotation> {
        decl.annotations.iter().find(|annotation| self.is(annotation, "lombok", name))
    }

    fn spring_on<'d>(&self, decl: &'d Decl, name: &str) -> Option<&'d Annotation> {
        decl.annotations.iter().find(|annotation| self.is(annotation, "org.springframework", name))
    }

    fn args(&self, annotation: &Annotation) -> String {
        annotation.args.clone().map(|args| self.text[args].split_whitespace().collect()).unwrap_or_default()
    }

    fn report(&mut self, range: Range<usize>, error: bool, message: String) {
        self.list.push(Diagnostic { start: position_of(self.text, range.start), end: position_of(self.text, range.end), message, error });
    }

    fn check_type(&mut self, ty: &Decl, properties: Option<&[String]>) {
        if self.lombok {
            self.check_lombok(ty);
        }
        if self.spring {
            self.check_spring(ty, properties);
        }
    }

    fn check_lombok(&mut self, ty: &Decl) {
        let (invalid, place): (&[&str], &str) = match ty.detail.as_str() {
            "interface" | "@interface" => (&["Data", "Value", "Builder", "Getter", "Setter", "With", "ToString", "EqualsAndHashCode", "NoArgsConstructor", "AllArgsConstructor", "RequiredArgsConstructor"], "un'interfaccia"),
            "enum" => (&["Data", "Value", "Builder", "Setter", "EqualsAndHashCode"], "un enum"),
            "record" => (&["Data", "Value", "Setter", "NoArgsConstructor"], "un record"),
            _ => (&[], ""),
        };
        for annotation in &ty.annotations {
            if let Some(name) = invalid.iter().find(|name| self.is(annotation, "lombok", name)) {
                self.report(annotation.at.clone(), true, format!("@{name} non vale su {place}: Lombok lo accetta sulle classi"));
            }
        }
        for field in ty.members(Kind::Field) {
            if let Some(setter) = self.lombok_on(field, "Setter").filter(|_| field.has("final")) {
                self.report(setter.at.clone(), false, "@Setter su un campo final: Lombok non genera il setter".into());
            }
            let default = field.annotations.iter().find(|annotation| annotation.name.ends_with("Builder.Default") && self.resolve(annotation.name.trim_end_matches(".Default")).is_some_and(|(package, _)| package == "lombok"));
            if let Some(default) = default.filter(|_| !field.flag) {
                self.report(default.at.clone(), true, "@Builder.Default vuole un valore iniziale".into());
            }
        }
        let parameters = ty.members(Kind::Method).flat_map(|method| &method.children);
        for decl in ty.members(Kind::Field).chain(parameters) {
            if let Some(non_null) = self.lombok_on(decl, "NonNull").filter(|_| PRIMITIVES.contains(&decl.detail.as_str())) {
                self.report(non_null.at.clone(), false, format!("@NonNull su un {} non ha effetto: un tipo primitivo non è mai null", decl.detail));
            }
        }
        if ty.detail != "class" {
            return;
        }
        let data = self.lombok_on(ty, "Data");
        let value = self.lombok_on(ty, "Value");
        if let (Some(_), Some(value)) = (data, value) {
            self.report(value.at.clone(), false, "@Data e @Value insieme: @Value rende i campi final e senza setter, @Data li vuole modificabili".into());
        }
        let unset: Vec<&str> = ty.members(Kind::Field).filter(|field| !field.has("static") && !field.flag && (field.has("final") || value.is_some())).map(|field| field.name.as_str()).collect();
        let no_args = self.lombok_on(ty, "NoArgsConstructor");
        if let Some(no_args) = no_args.filter(|no_args| !unset.is_empty() && !self.args(no_args).contains("force=true")) {
            self.report(no_args.at.clone(), true, format!("@NoArgsConstructor con campi final senza valore ({}): serve force = true o un valore iniziale", unset.join(", ")));
        }
        let constructors = ty.members(Kind::Method).any(|method| method.detail.is_empty() && method.name == ty.name);
        let other = no_args.is_some() || self.lombok_on(ty, "RequiredArgsConstructor").is_some();
        if let Some(builder) = self.lombok_on(ty, "Builder").filter(|_| other && !constructors && self.lombok_on(ty, "AllArgsConstructor").is_none()) {
            self.report(builder.at.clone(), true, "@Builder vuole un costruttore con tutti i campi: con @NoArgsConstructor o @RequiredArgsConstructor aggiungi @AllArgsConstructor".into());
        }
        if ty.members(Kind::Field).any(|field| field.name == "log") {
            if let Some((annotation, name)) = ty.annotations.iter().find_map(|annotation| LOGGERS.iter().find(|name| self.is(annotation, "lombok", name)).map(|name| (annotation, name))) {
                self.report(annotation.at.clone(), true, format!("@{name} crea il campo log, che esiste già"));
            }
        }
        if ty.flag {
            let equals = self.lombok_on(ty, "EqualsAndHashCode");
            let culprit = match equals {
                Some(equals) => (!self.args(equals).contains("callSuper")).then_some(equals),
                None => data.or(value),
            };
            if let Some(culprit) = culprit {
                self.report(culprit.at.clone(), false, "equals e hashCode senza i campi della superclasse: scrivi @EqualsAndHashCode(callSuper = true), o false se è voluto".into());
            }
        }
    }

    fn check_spring(&mut self, ty: &Decl, properties: Option<&[String]>) {
        let place = if ty.detail == "interface" { Some("un'interfaccia") } else { (ty.detail == "class" && ty.has("abstract")).then_some("una classe astratta") };
        if let Some(place) = place {
            for annotation in &ty.annotations {
                if let Some(name) = STEREOTYPES.iter().chain(&["Configuration"]).find(|name| self.is(annotation, "org.springframework", name)) {
                    self.report(annotation.at.clone(), false, format!("Spring non crea bean da {place}: @{name} va sulla classe concreta"));
                }
            }
        }
        let configuration = self.spring_on(ty, "Configuration");
        let proxied = configuration.is_some_and(|configuration| !self.args(configuration).contains("proxyBeanMethods=false"));
        if let Some(configuration) = configuration.filter(|_| proxied && ty.has("final")) {
            self.report(configuration.at.clone(), true, "Una @Configuration non può essere final: Spring ne crea una sottoclasse (o proxyBeanMethods = false)".into());
        }
        for method in ty.members(Kind::Method) {
            if let Some(bean) = self.spring_on(method, "Bean").filter(|_| proxied && (method.has("private") || method.has("final"))) {
                self.report(bean.at.clone(), true, "Un metodo @Bean di una @Configuration non può essere private né final".into());
            }
            let modifier = ["private", "static", "final"].into_iter().find(|modifier| method.has(modifier));
            for annotation in &method.annotations {
                if let Some((name, modifier)) = PROXIED.iter().find(|name| self.is(annotation, "org.springframework", name)).zip(modifier) {
                    self.report(annotation.at.clone(), false, format!("@{name} su un metodo {modifier} non ha effetto: il proxy di Spring non lo intercetta"));
                }
            }
            self.check_mapping(ty, method);
            for parameter in &method.children {
                if let Some(value) = self.spring_on(parameter, "Value") {
                    self.check_value(value, properties);
                }
            }
        }
        for field in ty.members(Kind::Field) {
            if let Some(autowired) = self.spring_on(field, "Autowired") {
                let (error, message) = if field.has("final") {
                    (true, "@Autowired su un campo final: Spring non può assegnarlo, usa il costruttore")
                } else if field.has("static") {
                    (false, "Spring non inietta i campi static")
                } else {
                    (false, "Iniezione nel campo: meglio un costruttore, con il campo final")
                };
                self.report(autowired.at.clone(), error, message.into());
            }
            if let Some(value) = self.spring_on(field, "Value") {
                if field.has("static") {
                    self.report(value.at.clone(), false, "Spring non inietta i campi static".into());
                }
                self.check_value(value, properties);
            }
        }
    }

    /// `${chiave}` senza default che non è in nessun file application.
    fn check_value(&mut self, annotation: &Annotation, properties: Option<&[String]>) {
        let (Some(properties), Some(args)) = (properties, annotation.args.clone()) else { return };
        let raw = &self.text[args.clone()];
        let mut from = 0;
        while let Some(open) = raw[from..].find("${").map(|at| from + at + 2) {
            let end = raw[open..].find(['}', ':', '$']).map_or(raw.len(), |at| open + at);
            from = end;
            let key = raw[open..end].trim();
            if key.is_empty() || raw[end..].starts_with([':', '$']) || properties.contains(&normalize(key)) {
                continue;
            }
            self.report(args.start + open..args.start + end, false, format!("Proprietà «{key}» non trovata in application.properties / .yml e senza default (${{{key}:valore}})"));
        }
    }

    /// Le stringhe date a uno degli attributi (`value` anche senza nome); None se non sono solo stringhe letterali.
    fn strings(&self, annotation: &Annotation, keys: &[&str]) -> Option<Vec<String>> {
        let Some(args) = annotation.args.clone() else { return Some(Vec::new()) };
        let mut parts = Vec::new();
        let (mut depth, mut start) = (0usize, args.start);
        for index in args.clone() {
            match self.m[index] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    parts.push(start..index);
                    start = index + 1;
                }
                _ => {}
            }
        }
        parts.push(start..args.end);
        let mut found = Vec::new();
        for part in parts {
            let masked = std::str::from_utf8(&self.m[part.clone()]).unwrap_or("");
            let (key, value) = match masked.find('=') {
                Some(at) => (masked[..at].trim(), part.start + at + 1..part.end),
                None => ("value", part),
            };
            if !keys.contains(&key) {
                continue;
            }
            if !self.m[value.clone()].iter().all(|byte| byte.is_ascii_whitespace() || b"\"{},".contains(byte)) {
                return None;
            }
            let quotes: Vec<usize> = value.filter(|&index| self.m[index] == b'"').collect();
            found.extend(quotes.chunks(2).filter(|pair| pair.len() == 2).map(|pair| self.text[pair[0] + 1..pair[1]].to_string()));
        }
        Some(found)
    }

    /// `{id}` del path (della classe e del metodo) contro i parametri `@PathVariable`.
    fn check_mapping(&mut self, ty: &Decl, method: &Decl) {
        let Some(mapping) = method.annotations.iter().find(|annotation| MAPPINGS.iter().any(|name| self.is(annotation, "org.springframework", name))) else { return };
        let class_paths = match self.spring_on(ty, "RequestMapping") {
            Some(annotation) => self.strings(annotation, &["value", "path"]),
            None => Some(Vec::new()),
        };
        let (Some(class_paths), Some(paths)) = (class_paths, self.strings(mapping, &["value", "path"])) else { return };
        let variables: BTreeSet<String> = class_paths.iter().chain(&paths).flat_map(|path| variables(path)).collect();
        let mut bound = BTreeSet::new();
        let mut checked = Vec::new();
        for parameter in &method.children {
            if self.spring_on(parameter, "ModelAttribute").is_some() {
                return;
            }
            let Some(annotation) = self.spring_on(parameter, "PathVariable") else { continue };
            // Una mappa riceve tutte le variabili del path.
            if parameter.detail.starts_with("Map") {
                return;
            }
            let Some(named) = self.strings(annotation, &["value", "name"]) else { return };
            let name = named.into_iter().next().unwrap_or_else(|| parameter.name.clone());
            bound.insert(name.clone());
            checked.push((annotation.at.clone(), name));
        }
        let listed = variables.iter().map(|variable| format!("{{{variable}}}")).collect::<Vec<_>>().join(", ");
        for (at, name) in checked {
            if !variables.contains(&name) {
                let message = if variables.is_empty() { format!("@PathVariable «{name}»: il path non ha variabili") } else { format!("@PathVariable «{name}» non è nel path, che ha {listed}") };
                self.report(at, true, message);
            }
        }
        let missing: Vec<String> = variables.iter().filter(|variable| !bound.contains(*variable)).map(|variable| format!("{{{variable}}}")).collect();
        if !missing.is_empty() {
            self.report(mapping.at.clone(), false, format!("{} nel path senza un parametro @PathVariable", missing.join(", ")));
        }
    }
}

/// Le variabili di un path di Spring: `{id}`, `{id:\d+}`, `{*resto}`.
fn variables(path: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut rest = path;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|at| open + at) else { break };
        let inner = rest[open + 1..close].trim_start_matches('*');
        list.push(inner.split(':').next().unwrap_or("").trim().to_string());
        rest = &rest[close + 1..];
    }
    list.retain(|name| !name.is_empty());
    list
}

/// Il binding di Spring non distingue maiuscole, trattini e trattini bassi (`my-prop`, `myProp`, `my_prop`).
fn normalize(key: &str) -> String {
    key.chars().filter(|ch| *ch != '-' && *ch != '_').flat_map(char::to_lowercase).collect()
}

fn collect_types<'d>(decls: &'d [Decl], out: &mut Vec<&'d Decl>) {
    for decl in decls.iter().filter(|decl| decl.kind == Kind::Type) {
        out.push(decl);
        collect_types(&decl.children, out);
    }
}

/// I problemi di Spring (se `spring`) e di Lombok (se `lombok`) nel file Java; `properties`: le chiavi dei file
/// application, None se il progetto non ne ha (allora `@Value` non si controlla).
pub fn check(text: &str, spring: bool, lombok: bool, properties: Option<&[String]>) -> Vec<Diagnostic> {
    let m = mask(text);
    let masked = String::from_utf8_lossy(&m).into_owned();
    let decls = Parser { text, m: &m, pos: 0 }.members(m.len(), false);
    let mut types = Vec::new();
    collect_types(&decls, &mut types);
    let properties: Option<Vec<String>> = properties.map(|keys| keys.iter().map(|key| normalize(key)).collect());
    let mut checker = Checker { text, imports: imports(&masked), m, spring, lombok, list: Vec::new() };
    for ty in types {
        checker.check_type(ty, properties.as_deref());
    }
    checker.list
}

/// Suggerimenti di Spring e Lombok al cursore: proprietà in `@Value("${`, nomi dopo `@` (insieme a quelli del
/// server), attributi dentro le parentesi di un'annotazione conosciuta.
pub fn completions(text: &str, caret: usize, spring: bool, lombok: bool, properties: impl FnOnce() -> Option<Vec<String>>) -> Option<Local> {
    let before = text.get(..caret)?;
    let line_start = before.rfind('\n').map_or(0, |at| at + 1);
    let line = &before[line_start..];
    if let Some(open) = line.rfind("${").filter(|_| spring) {
        let typed = &line[open + 2..];
        let inside = line[..open].rfind('"').zip(line[..open].rfind("@Value")).is_some_and(|(quote, value)| quote > value);
        if inside && typed.bytes().all(|byte| is_ident(byte) || b".-[]".contains(&byte)) {
            return Some(Local { start: line_start + open + 2, labels: properties()?, merge: false });
        }
    }
    let m = mask(before);
    let word_start = caret - m.iter().rev().take_while(|byte| is_ident(**byte)).count();
    if word_start > 0 && m[word_start - 1] == b'@' {
        let names: BTreeSet<&str> = tables(spring, lombok).flat_map(|(_, list)| list.iter().map(|(name, _)| *name)).collect();
        return Some(Local { start: word_start, labels: names.into_iter().map(str::to_string).collect(), merge: true });
    }
    let mut before_word = word_start;
    while before_word > 0 && m[before_word - 1].is_ascii_whitespace() {
        before_word -= 1;
    }
    if before_word == 0 || !matches!(m[before_word - 1], b'(' | b',') {
        return None;
    }
    // La parentesi aperta più vicina deve essere quella di un'annotazione.
    let mut depth = 0usize;
    let open = (before_word.saturating_sub(4000)..before_word).rev().find(|&index| match m[index] {
        b')' | b']' | b'}' => {
            depth += 1;
            false
        }
        b'(' | b'[' | b'{' if depth > 0 => {
            depth -= 1;
            false
        }
        b'(' | b'[' | b'{' => true,
        _ => false,
    })?;
    if m[open] != b'(' {
        return None;
    }
    let mut end = open;
    while end > 0 && m[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && (is_ident(m[start - 1]) || m[start - 1] == b'.') {
        start -= 1;
    }
    if start == 0 || m[start - 1] != b'@' {
        return None;
    }
    let name = before[start..end].rsplit('.').next()?;
    let attributes: BTreeSet<&str> = tables(spring, lombok).flat_map(|(_, list)| list.iter()).filter(|(candidate, _)| *candidate == name).flat_map(|(_, attributes)| attributes.iter().copied()).collect();
    (!attributes.is_empty()).then(|| Local { start: word_start, labels: attributes.into_iter().map(str::to_string).collect(), merge: false })
}

fn properties_keys(content: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut continued = false;
    for line in content.lines() {
        let was_continued = continued;
        continued = line.ends_with('\\');
        let line = line.trim_start();
        if was_continued || line.is_empty() || line.starts_with(['#', '!']) {
            continue;
        }
        let end = line.find(|ch: char| ch == '=' || ch == ':' || ch.is_whitespace()).unwrap_or(line.len());
        keys.push(line[..end].to_string());
    }
    keys
}

/// Le chiavi di un YAML a mappe annidate, con il percorso a punti; le liste non si seguono.
fn yaml_keys(content: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for line in content.lines() {
        if line.starts_with("---") {
            stack.clear();
            continue;
        }
        let item = line.trim_start();
        if item.is_empty() || item.starts_with(['#', '-']) {
            continue;
        }
        let indent = line.len() - item.len();
        let Some(colon) = item.find(": ").or_else(|| item.ends_with(':').then(|| item.len() - 1)) else { continue };
        let key = item[..colon].trim().trim_matches(['"', '\'']);
        while stack.last().is_some_and(|(level, _)| *level >= indent) {
            stack.pop();
        }
        let path: Vec<&str> = stack.iter().map(|(_, key)| key.as_str()).chain([key]).collect();
        keys.push(path.join("."));
        stack.push((indent, key.to_string()));
    }
    keys
}

/// Chiavi dei file application*/bootstrap* (.properties, .yml, .yaml) in src/main/resources e nella sua cartella
/// config; None se il progetto non ne ha.
pub fn properties(root: &Path) -> Option<Vec<String>> {
    let resources = root.join("src").join("main").join("resources");
    let mut keys = Vec::new();
    let mut found = false;
    for dir in [resources.clone(), resources.join("config")] {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !(name.starts_with("application") || name.starts_with("bootstrap")) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(entry.path()) else { continue };
            if name.ends_with(".properties") {
                keys.extend(properties_keys(&content));
            } else if name.ends_with(".yml") || name.ends_with(".yaml") {
                keys.extend(yaml_keys(&content));
            } else {
                continue;
            }
            found = true;
        }
    }
    keys.sort();
    keys.dedup();
    found.then_some(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(text: &str, properties: Option<&[String]>) -> Vec<(String, bool)> {
        check(text, true, true, properties).into_iter().map(|diagnostic| (diagnostic.message, diagnostic.error)).collect()
    }

    fn has(list: &[(String, bool)], part: &str, error: bool) -> bool {
        list.iter().any(|(message, is_error)| message.contains(part) && *is_error == error)
    }

    #[test]
    fn checks_lombok_combinations() {
        let text = "package a;\nimport lombok.*;\nimport lombok.extern.slf4j.Slf4j;\n\
            @Data @Value public class A extends B {\n  private final String name;\n  @Setter private final int n = 1;\n  @NonNull int count;\n}\n\
            @Builder @NoArgsConstructor class C {\n  @Builder.Default private int x;\n}\n\
            @Data interface I {}\n\
            @Slf4j class L { private static final Object log = null; void f(@NonNull String s) {} }\n\
            @NoArgsConstructor(force = true) class F { final int a; }\n\
            @Data @EqualsAndHashCode(callSuper = false) class E extends B {}\n";
        let list = messages(text, None);
        assert!(has(&list, "@Data e @Value insieme", false));
        assert!(has(&list, "equals e hashCode", false));
        assert!(has(&list, "@Setter su un campo final", false));
        assert!(has(&list, "@NonNull su un int", false));
        assert!(has(&list, "@Builder vuole un costruttore", true), "{list:?}");
        assert!(has(&list, "@Builder.Default vuole un valore iniziale", true));
        assert!(has(&list, "@Data non vale su un'interfaccia", true));
        assert!(has(&list, "@Slf4j crea il campo log", true));
        assert_eq!(list.iter().filter(|(message, _)| message.contains("equals e hashCode")).count(), 1, "{list:?}");
        assert!(!list.iter().any(|(message, _)| message.contains("NoArgsConstructor con campi") && message.contains("(a)")), "{list:?}");
        assert!(!has(&list, "@NonNull su un String", false));
    }

    #[test]
    fn ignores_annotations_from_other_packages() {
        let text = "import com.acme.Data;\nimport org.springframework.beans.factory.annotation.Value;\nimport lombok.*;\n@Data interface I { }\nclass V { @Value(\"${missing}\") String s; }\n";
        let list = messages(text, Some(&["present".to_string()]));
        assert!(!has(&list, "@Data non vale", true), "{list:?}");
        assert!(has(&list, "Proprietà «missing»", false), "{list:?}");
    }

    #[test]
    fn checks_spring_annotations() {
        let text = "import org.springframework.web.bind.annotation.*;\nimport org.springframework.beans.factory.annotation.*;\n\
            import org.springframework.transaction.annotation.Transactional;\nimport org.springframework.context.annotation.*;\nimport org.springframework.stereotype.Service;\n\
            @RestController @RequestMapping(\"/users/{userId}\")\nclass UserController {\n\
            @Autowired private final Repo repo;\n  @Autowired Repo other;\n\
            @Value(\"${server.port}\") int port;\n  @Value(\"${app.missing}\") String missing;\n  @Value(\"${app.with:default}\") String ok;\n\
            @GetMapping(\"/orders/{orderId}\")\n  public Order get(@PathVariable Long userId, @PathVariable(\"orderId\") Long id, @PathVariable Long extra) { return null; }\n\
            @PostMapping(value = {\"/a/{x}\"})\n  void post() {}\n\
            @GetMapping(BASE + \"/{x}\")\n  void dynamic() {}\n\
            @Transactional\n  private void save() {}\n}\n\
            @Configuration final class Config { @Bean private Repo repo() { return null; } }\n\
            @Service interface S {}\n";
        let properties = vec!["server.port".to_string(), "app.other".to_string()];
        let list = messages(text, Some(&properties));
        assert!(has(&list, "@Autowired su un campo final", true), "{list:?}");
        assert!(has(&list, "Iniezione nel campo", false));
        assert!(has(&list, "Proprietà «app.missing»", false));
        assert!(!has(&list, "server.port", false) && !has(&list, "app.with", false));
        assert!(has(&list, "@PathVariable «extra» non è nel path", true), "{list:?}");
        assert!(!list.iter().any(|(message, _)| message.contains("«userId»") || message.contains("«orderId»")), "{list:?}");
        assert!(has(&list, "{userId}, {x} nel path senza un parametro", false), "{list:?}");
        assert_eq!(list.iter().filter(|(message, _)| message.contains("nel path senza")).count(), 1, "{list:?}");
        assert!(has(&list, "@Transactional su un metodo private", false));
        assert!(has(&list, "Una @Configuration non può essere final", true));
        assert!(has(&list, "Un metodo @Bean", true));
        assert!(has(&list, "Spring non crea bean da un'interfaccia", false));
    }

    #[test]
    fn completes_names_attributes_and_properties() {
        let complete = |text: &str| {
            let caret = text.find('|').unwrap();
            let text = text.replace('|', "");
            completions(&text, caret, true, true, || Some(vec!["server.port".into(), "spring.application.name".into()]))
        };
        let local = complete("class A {\n  @Bui|").unwrap();
        assert!(local.merge && local.labels.contains(&"Builder".to_string()) && local.labels.contains(&"Autowired".to_string()));
        let local = complete("@GetMapping(produces = \"x\", |").unwrap();
        assert!(!local.merge && local.labels.contains(&"consumes".to_string()) && !local.labels.contains(&"method".to_string()));
        let local = complete("  @Value(\"${serv|").unwrap();
        assert_eq!((local.start, local.labels.len()), (12, 2));
        assert!(complete("  @RequestMapping(value = {\"a\", |").is_none());
        assert!(complete("  /** @para|").is_none());
        assert!(complete("  call(|").is_none());
    }

    #[test]
    fn reads_property_and_yaml_keys() {
        assert_eq!(properties_keys("# c\nserver.port=8080\napp.name : x\nlong=a\\\n  b=c\n"), vec!["server.port", "app.name", "long"]);
        let yaml = "server:\n  port: 8080\n  servlet:\n    context-path: /x\n# c\nspring:\n  datasource:\n    url: jdbc\n---\nother: 1\n";
        assert_eq!(yaml_keys(yaml), vec!["server", "server.port", "server.servlet", "server.servlet.context-path", "spring", "spring.datasource", "spring.datasource.url", "other"]);
        assert_eq!(normalize("app.Some-Thing_x"), "app.somethingx");
        assert_eq!(variables("/a/{id}/{name:[a-z]+}/{*rest}"), vec!["id", "name", "rest"]);
    }
}
