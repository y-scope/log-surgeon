#ifndef LOG_SURGEON_ALIASES_HPP
#define LOG_SURGEON_ALIASES_HPP

#include <cstdint>
#include <functional>
#include <memory>
#include <variant>

namespace log_surgeon {
class ItemSet;
class NonTerminal;
class ParserAST;
class Production;
class Token;

using MatchedSymbol = std::variant<Token, NonTerminal>;
using SemanticRule = std::function<std::unique_ptr<ParserAST>(NonTerminal*)>;
using Action = std::variant<bool, ItemSet*, Production*>;

using capture_id_t = uint32_t;
using reg_id_t = uint32_t;
using rule_id_t = uint32_t;
using tag_id_t = uint32_t;
}  // namespace log_surgeon

#endif  // LOG_SURGEON_ALIASES_HPP
