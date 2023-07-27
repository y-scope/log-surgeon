#ifndef LOG_SURGEON_LALR1_PARSER_HPP
#define LOG_SURGEON_LALR1_PARSER_HPP

#include <cassert>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <list>
#include <map>
#include <optional>
#include <set>
#include <stack>
#include <tuple>
#include <type_traits>
#include <unordered_map>
#include <variant>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/Parser.hpp>

namespace log_surgeon {

class ParserAST;
class NonTerminal;

template <typename T>
class ParserValue;

struct Production;
struct Item;
struct ItemSet;

using SemanticRule = std::function<std::unique_ptr<ParserAST>(NonTerminal*)>;
using Action = std::variant<bool, ItemSet*, Production*>;

class ParserAST {
public:
    virtual ~ParserAST() = 0;

    template <typename T>
    auto get() -> T& {
        return static_cast<ParserValue<T>*>(this)->m_value;
    }
};

template <typename T>
class ParserValue : public ParserAST {
public:
    T m_value;

    explicit ParserValue(T val) : m_value(std::move(val)) {}
};

using MatchedSymbol = std::variant<Token, NonTerminal>;

template <class... Ts>
struct Overloaded : Ts... {
    using Ts::operator()...;
};
template <class... Ts>
Overloaded(Ts...) -> Overloaded<Ts...>;

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
        return &std::get<Token>(NonTerminal::m_all_children[m_children_start + i]);
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
        return &std::get<NonTerminal>(NonTerminal::m_all_children[m_children_start + i]);
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

/**
 * Structure representing a production of the form "m_head -> {m_body}".
 * The code fragment to execute upon reducing "{m_body} -> m_head" is
 * m_semantic_rule, which is purely a function of the MatchedSymbols for
 * {m_body}. m_index is the productions position in the parsers production
 * vector.
 */
struct Production {
public:
    /**
     * Returns if the production is an epsilon production. An epsilon production
     * has nothing on its LHS (i.e., HEAD -> {})
     * @return bool
     */
    [[nodiscard]] auto is_epsilon() const -> bool { return this->m_body.empty(); }

    uint32_t m_index;
    uint32_t m_head;
    std::vector<uint32_t> m_body;
    SemanticRule m_semantic_rule;
};

/**
 * Structure representing an item in a LALR1 state.
 * An item (1) is associated with a m_production and a single m_lookahead which
 * is an input symbol (character) that can follow the m_production, and (2)
 * tracks the current matching progress of its associated m_production, where
 * everything exclusively to the left of m_dot is already matched.
 */
struct Item {
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
        return this->m_dot == this->m_production->m_body.size();
    }

    /**
     * Returns the next unmatched symbol in the production based on the dot.
     * @return uint32_t
     */
    [[nodiscard]] auto next_symbol() const -> uint32_t {
        return this->m_production->m_body.at(this->m_dot);
    }

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
struct ItemSet {
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

template <typename NFAStateType, typename DFAStateType>
class LALR1Parser : public Parser<NFAStateType, DFAStateType> {
public:
    LALR1Parser();

    /**
     * Add a lexical rule to m_lexer
     * @param name
     * @param rule
     */
    auto
    add_rule(std::string const& name, std::unique_ptr<finite_automata::RegexAST<NFAStateType>> rule)
            -> void override;

    /**
     * Calls add_rule with the given RegexASTGroup
     * @param name
     * @param rule_char
     */
    auto add_token_group(
            std::string const& name,
            std::unique_ptr<finite_automata::RegexASTGroup<NFAStateType>> rule_group
    ) -> void;

    /**
     * Constructs a RegexASTCat and calls add_rule
     * @param name
     * @param chain
     */
    auto add_token_chain(std::string const& name, std::string const& chain) -> void;

    /**
     * Adds productions (syntax rule) to the parser
     * @param head
     * @param body
     * @param semantic_rule
     * @return uint32_t
     */
    auto add_production(
            std::string const& head,
            std::vector<std::string> const& body,
            SemanticRule semantic_rule
    ) -> uint32_t;

    /**
     * Generate the LALR1 parser (use after all the lexical rules and
     * productions have been added)
     */
    auto generate() -> void;

    // TODO: add throws to function headers
    /**
     * Parse an input (e.g. file)
     * @param reader
     * @return NonTerminal
     */
    auto parse(Reader& reader) -> NonTerminal;

protected:
    /**
     * Reset the parser to start a new parsing (set state to root, reset
     * buffers, reset vars tracking positions)
     */
    auto reset() -> void;

    /**
     * Return an error string based on the current error state, matched_stack,
     * and next_symbol in the parser
     * @param reader
     * @return std::string
     */
    auto report_error() -> std::string;

    /* Lexer<NFAStateType, DFAStateType> m_lexer; */
    std::stack<MatchedSymbol> m_parse_stack_matches;
    std::stack<ItemSet*> m_parse_stack_states;
    ItemSet* m_root_item_set_ptr{nullptr};
    std::optional<Token> m_next_token;
    std::vector<std::unique_ptr<Production>> m_productions;
    std::unordered_map<std::string, std::map<std::vector<std::string>, Production*>>
            m_productions_map;
    std::unordered_map<uint32_t, std::vector<Production*>> m_non_terminals;
    uint32_t m_root_production_id{0};
    ParserInputBuffer m_input_buffer;

private:
    /**
     * Generate LR0 kernels based on the productions in m_productions
     */
    auto generate_lr0_kernels() -> void;

    /**
     * Perform closure for the specified item_set based on its kernel
     * @param item_set
     */
    auto generate_lr0_closure(ItemSet* item_set_ptr) -> void;

    /**
     * Helper function for doing the closure on a specified item set
     * @param item_set_ptr
     * @param item
     * @param next_symbol
     * @return bool
     */
    auto lr_closure_helper(ItemSet* item_set_ptr, Item const* item, uint32_t* next_symbol) -> bool;

    /**
     * Return the next state (ItemSet) based on the current state (ItemSet) and
     * input symbol
     * @return ItemSet*
     */
    auto go_to(ItemSet* /*from_item_set*/, uint32_t const& /*next_symbol*/) -> ItemSet*;

    /**
     * Generate m_firsts, which specify for each symbol, all possible prefixes
     */
    auto generate_first_sets() -> void;

    /**
     * Generate kernels for LR1 item sets based on LR0 item sets
     */
    auto generate_lr1_item_sets() -> void;

    /**
     * Generate closure for a specified LR1 item set
     * @param item_set_ptr
     */
    auto generate_lr1_closure(ItemSet* item_set_ptr) -> void;

    /**
     * Generating parsing table and goto table for LALR1 parser based on
     * state-symbol pair generate_lalr1_goto() + generate_lalr1_action()
     */
    auto generate_lalr1_parsing_table() -> void;

    /**
     * Generating the goto table for LARL1 parser specifying which state
     * (ItemSet) to transition to based on state-symbol pair Does nothing (its
     * already done in an earlier step)
     */
    auto generate_lalr1_goto() -> void;

    /**
     *  Generating the action table for LARL1 parser specifying which action to
     *  perform based on state-symbol pair
     */
    auto generate_lalr1_action() -> void;

    /**
     * Use the previous symbol from the lexer if unused, otherwise request the
     * next symbol from the lexer
     * @return Token
     */
    auto get_next_symbol() -> Token;

    /**
     * Tries all symbols in the language that the next token may be until the
     * first non-error symbol is tried
     * @param next_token
     * @param accept
     * @return bool
     */
    auto parse_advance(Token& next_token, bool* accept) -> bool;

    /**
     * Perform an action and state transition based on the current state
     * (ItemSet) and the type_id (current symbol interpretation of the
     * next_token)
     * @param type_id
     * @param next_token
     * @param accept
     * @return bool
     */
    auto parse_symbol(uint32_t const& type_id, Token& next_token, bool* accept) -> bool;

    /**
     * Get the current line up to the error symbol
     * @param parse_stack_matches
     * @return std::string
     */
    static auto get_input_after_last_newline(std::stack<MatchedSymbol>& parse_stack_matches)
            -> std::string;

    /**
     * Get the current line after the error symbol
     * @param reader
     * @param error_token
     * @return std::string
     */
    auto get_input_until_next_newline(Token* error_token) -> std::string;

    auto symbol_is_token(uint32_t s) -> bool { return m_terminals.find(s) != m_terminals.end(); }

    std::set<uint32_t> m_terminals;
    std::set<uint32_t> m_nullable;
    std::map<std::set<Item>, std::unique_ptr<ItemSet>> m_lr0_item_sets;
    std::map<std::set<Item>, std::unique_ptr<ItemSet>> m_lr1_item_sets;
    std::unordered_map<uint32_t, std::set<uint32_t>> m_firsts;
    std::unordered_map<Production*, std::set<uint32_t>> m_spontaneous_map;
    std::map<Item, std::set<Item>> m_propagate_map;
    std::unordered_map<uint32_t, std::map<uint32_t, uint32_t>> m_go_to_table;
};
}  // namespace log_surgeon

#include "LALR1Parser.tpp"

#endif  // LOG_SURGEON_LALR1_PARSER_HPP
