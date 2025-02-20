#ifndef LOG_SURGEON_NONTERMINAL_HPP
#define LOG_SURGEON_NONTERMINAL_HPP

#include <cassert>
#include <cstdint>
#include <memory>

#include <log_surgeon/ParserAst.hpp>
#include <log_surgeon/Production.hpp>
#include <log_surgeon/Token.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {

class NonTerminal {
public:
    NonTerminal() : m_children_start(0), m_production(nullptr), m_ast(nullptr) {}

    explicit NonTerminal(Production*);

    /**
     * Return the ith child's (body of production) MatchedSymbol as a Token.
     * Note: only children are needed (and stored) for performing semantic
     * actions (for the AST)
     * @param i
     * @return Token*
     */
    [[nodiscard]] auto token_cast(uint32_t i) const -> Token* {
        assert(i < cSizeOfAllChildren);
        return &std::get<Token>(m_all_children[m_children_start + i]);
    }

    /**
     * Return the ith child's (body of production) MatchedSymbol as a
     * NonTerminal. Note: only children are needed (and stored) for performing
     * semantic actions (for the AST)
     * @param i
     * @return NonTerminal*
     */
    [[nodiscard]] auto non_terminal_cast(uint32_t i) const -> NonTerminal* {
        assert(i < cSizeOfAllChildren);
        return &std::get<NonTerminal>(m_all_children[m_children_start + i]);
    }

    /**
     * Return the AST that relates this non_terminal's children together (based
     * on the production/syntax-rule that was determined to have generated them)
     * @return std::unique_ptr<ParserAST>
     */
    auto get_parser_ast() -> std::unique_ptr<ParserAST>& { return m_ast; }

    static MatchedSymbol m_all_children[];
    static uint32_t m_next_children_start;
    uint32_t m_children_start;
    Production* m_production;
    std::unique_ptr<ParserAST> m_ast;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_NONTERMINAL_HPP
