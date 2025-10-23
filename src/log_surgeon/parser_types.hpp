#ifndef LOG_SURGEON_PARSER_TYPES_HPP
#define LOG_SURGEON_PARSER_TYPES_HPP

#include <cassert>
#include <cstdint>
#include <functional>
#include <memory>
#include <set>
#include <tuple>
#include <unordered_map>
#include <utility>
#include <variant>
#include <vector>

#include <log_surgeon/ParserAst.hpp>
#include <log_surgeon/Token.hpp>

namespace log_surgeon {
class ItemSet;
class NonTerminal;
class ParserAST;
class Production;
class Token;

using MatchedSymbol = std::variant<Token, NonTerminal>;
using SemanticRule = std::function<std::unique_ptr<ParserAST>(NonTerminal*)>;
using Action = std::variant<bool, ItemSet*, Production*>;

/**
 * Structure representing a production of the form "m_head -> {m_body}". The code fragment to
 * execute upon reducing "{m_body} -> m_head" is m_semantic_rule, which is purely a function of the
 * MatchedSymbols for {m_body}. m_index is the productions position in the parsers production
 * vector.
 */
class Production {
public:
    /**
     * Returns if the production is an epsilon production. An epsilon production has nothing on its
     * LHS (i.e., HEAD -> {})
     * @return bool
     */
    [[nodiscard]] auto is_epsilon() const -> bool { return m_body.empty(); }

    uint32_t m_index;
    uint32_t m_head;
    std::vector<uint32_t> m_body;
    SemanticRule m_semantic_rule;
};

/**
 * Represents a non-terminal symbol in the parser, which corresponds to a production rule. A
 * `NonTerminal` is associated with a specific `Production` and maintains references to its children
 * in the parse tree. These children are stored as `MatchedSymbols` and can be cast to either
 * `Token` or `NonTerminal` types for semantic processing.
 *
 * The `NonTerminal` also maintains a reference to its corresponding AST node, which represents the
 * syntactic structure derived from this production. The AST is constructed based on the semantic
 * rules associated with the production.
 */
class NonTerminal {
public:
    NonTerminal() : NonTerminal(nullptr) {}

    explicit NonTerminal(Production* production) : m_production(production), m_ast(nullptr) {}

    /**
     * Return the ith child's (body of production) MatchedSymbol as a Token.
     * Note: only children are needed (and stored) for performing semantic actions (for the AST)
     * @param i
     * @return Token*
     */
    [[nodiscard]] auto token_cast(uint32_t i) -> Token& { return std::get<Token>(m_symbols.at(i)); }

    /**
     * Return the ith child's (body of production) MatchedSymbol as a NonTerminal. Note: only
     * children are needed (and stored) for performing semantic actions (for the AST)
     * @param i
     * @return NonTerminal*
     */
    [[nodiscard]] auto non_terminal_cast(uint32_t i) -> NonTerminal& {
        return std::get<NonTerminal>(m_symbols.at(i));
    }

    /**
     * Return a reference to the AST that relates this non_terminal's children together (based on
     * the production/syntax-rule that was determined to have generated them).
     * @return ParserAST&
     */
    auto get_parser_ast() -> ParserAST& { return *m_ast; }

    /**
     * Release and return the AST that relates this non_terminal's children together (based on the
     * production/syntax-rule that was determined to have generated them).
     * @return std::unique_ptr<ParserAST>
     */
    auto release_parser_ast() -> std::unique_ptr<ParserAST> { return std::move(m_ast); }

    /**
     * Store the specified ParserAST.
     * @param ast
     */
    auto set_ast(std::unique_ptr<ParserAST> ast) -> void { m_ast = std::move(ast); }

    /**
     * Move the ith child's (body of production) MatchedSymbol out of the symbols container.
     * @param i
     * @return MatchedSymbol&&
     */
    [[nodiscard]] auto move_symbol(uint32_t i) -> MatchedSymbol&& {
        return std::move(m_symbols.at(i));
    }

    /**
     * Resize the symbols container allowing for unordered insertion.
     * @param i
     * @return MatchedSymbol&&
     */
    auto resize_symbols(uint32_t size) -> void { m_symbols.resize(size); }

    /**
     * Store the specified MatchedSymbol as the ith element.
     * @param ast
     */
    auto set_symbol(uint32_t i, MatchedSymbol symbol) -> void {
        m_symbols.at(i) = std::move(symbol);
    }

    [[nodiscard]] auto get_production() -> Production* { return m_production; }

private:
    std::vector<MatchedSymbol> m_symbols;
    Production* m_production;
    std::unique_ptr<ParserAST> m_ast;
};

/**
 * Structure representing an item in a LALR1 state.
 * An item (1) is associated with a m_production and a single m_lookahead which
 * is an input symbol (character) that can follow the m_production, and (2)
 * tracks the current matching progress of its associated m_production, where
 * everything exclusively to the left of m_dot is already matched.
 */
class Item {
public:
    Item() = default;

    Item(Production* p, uint32_t d, uint32_t t) : m_production(p), m_dot(d), m_lookahead(t) {}

    /**
     * Comparison operator for tie-breakers (not 100% sure where this is used)
     * @param lhs
     * @param rhs
     * @return bool
     */
    friend auto operator<(Item const& lhs, Item const& rhs) -> bool {
        return std::tie(lhs.m_production->m_index, lhs.m_dot, lhs.m_lookahead)
               < std::tie(rhs.m_production->m_index, rhs.m_dot, rhs.m_lookahead);
    }

    /**
     * Returns if the item has a dot at the end. This indicates the production
     * associated with the item has already been fully matched.
     * @return bool
     */
    [[nodiscard]] auto has_dot_at_end() const -> bool {
        return m_dot == m_production->m_body.size();
    }

    /**
     * Returns the next unmatched symbol in the production based on the dot.
     * @return uint32_t
     */
    [[nodiscard]] auto next_symbol() const -> uint32_t { return m_production->m_body.at(m_dot); }

    Production* m_production;
    uint32_t m_dot;
    uint32_t m_lookahead;  // for LR0 items, `m_lookahead` is unused
};

/**
 * Structure representing an LALR1 state, a collection of items.
 * The m_kernel is sufficient for fully representing the state, but m_closure is
 * useful for computations. m_next indicates what state (ItemSet) to transition
 * to based on the symbol received from the lexer m_actions is the action to
 * perform based on the symbol received from the lexer.
 */
class ItemSet {
public:
    /**
     * Comparison operator for tie-breakers (not 100% sure where this is used)
     * @param lhs
     * @param rhs
     * @return bool
     */
    friend auto operator<(ItemSet const& lhs, ItemSet const& rhs) -> bool {
        return lhs.m_kernel < rhs.m_kernel;
    }

    auto empty() const -> bool { return m_kernel.empty(); }

    uint32_t m_index = -1;
    std::set<Item> m_kernel;
    std::set<Item> m_closure;
    std::unordered_map<uint32_t, ItemSet*> m_next;
    std::vector<Action> m_actions;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_PARSER_TYPES_HPP
