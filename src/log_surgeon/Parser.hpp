#ifndef LOG_SURGEON_PARSER_HPP
#define LOG_SURGEON_PARSER_HPP

#include <memory>
#include <string>

#include <log_surgeon/Lexer.hpp>

#include "finite_automata/RegexAST.hpp"

namespace log_surgeon {

template <typename NFAStateType, typename DFAStateType>
class Parser {
public:
    Parser();

    virtual ~Parser() = default;

    Parser(Parser const&) = delete;
    auto operator=(Parser const&) -> Parser& = delete;
    Parser(Parser&&) noexcept = delete;
    auto operator=(Parser&&) noexcept -> Parser& = delete;

    virtual auto add_rule(
            std::string const& name,
            std::unique_ptr<finite_automata::RegexAST<NFAStateType>> rule
    ) -> void;

    auto add_token(std::string const& name, char rule_char) -> void;

    Lexer<NFAStateType, DFAStateType> m_lexer;
};
}  // namespace log_surgeon

#include "Parser.tpp"

#endif  // LOG_SURGEON_PARSER_HPP
