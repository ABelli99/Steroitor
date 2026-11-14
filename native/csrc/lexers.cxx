// Catalogo ridotto di Lexilla: solo i lexer dei linguaggi che l'app riconosce.
#include <cstring>
#include <string>
#include <string_view>
#include <vector>
#include <map>

#include "ILexer.h"
#include "Scintilla.h"
#include "SciLexer.h"
#include "PropSetSimple.h"
#include "WordList.h"
#include "LexerModule.h"

using namespace Lexilla;

extern const LexerModule lmBatch, lmCPP, lmCss, lmDiff, lmHTML, lmXML, lmJSON, lmMarkdown, lmNull, lmPowerShell, lmProps,
    lmPython, lmRust, lmSQL, lmTOML, lmYAML;

static const LexerModule *const modules[] = {
    &lmBatch, &lmCPP, &lmCss, &lmDiff, &lmHTML, &lmXML, &lmJSON, &lmMarkdown, &lmNull, &lmPowerShell, &lmProps,
    &lmPython, &lmRust, &lmSQL, &lmTOML, &lmYAML,
};

extern "C" void *steroitor_create_lexer(const char *name) {
    for (const LexerModule *module : modules) {
        if (module->languageName && std::strcmp(module->languageName, name) == 0) return module->Create();
    }
    return nullptr;
}
