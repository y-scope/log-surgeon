#ifndef LOG_SURGEON_LALR1_PARSER_TPP
#define LOG_SURGEON_LALR1_PARSER_TPP

#include <cstddef>
#include <functional>
#include <iostream>
#include <optional>

#include <log_surgeon/Constants.hpp>

namespace log_surgeon {
namespace {
[[maybe_unused]] auto get_line_num(MatchedSymbol& top_symbol) -> uint32_t {
    std::optional<uint32_t> line_num{std::nullopt};
    std::stack<MatchedSymbol> symbols;
    symbols.push(std::move(top_symbol));
    while (std::nullopt == line_num) {
        assert(!symbols.empty());
        MatchedSymbol& curr_symbol = symbols.top();
        std::visit(
                Overloaded{
                        [&line_num](Token& token) { line_num = token.m_line; },
                        [&symbols](NonTerminal& m) {
                            for (size_t i = 0; i < m.m_production->m_body.size(); i++) {
                                symbols.push(std::move(
                                        NonTerminal::m_all_children[m.m_children_start + i]
                                ));
                            }
                        }
                },
                curr_symbol
        );
        symbols.pop();
    }
    return *line_num;
}

[[maybe_unused]] auto unescape(char const& c) -> std::string {
    switch (c) {
        case '\t':
            return "\\t";
        case '\r':
            return "\\r";
        case '\n':
            return "\\n";
        case '\v':
            return "\\v";
        case '\f':
            return "\\f";
        default:
            return {c};
    }
}
}  // namespace

template <typename NFAStateType, typename DFAStateType>
LALR1Parser<NFAStateType, DFAStateType>::LALR1Parser() {
    m_terminals.insert((uint32_t)SymbolId::TokenEnd);
    m_terminals.insert((uint32_t)SymbolId::TokenUncaughtString);
    m_terminals.insert((uint32_t)SymbolId::TokenInt);
    m_terminals.insert((uint32_t)SymbolId::TokenFloat);
    m_terminals.insert((uint32_t)SymbolId::TokenHex);
    m_terminals.insert((uint32_t)SymbolId::TokenFirstTimestamp);
    m_terminals.insert((uint32_t)SymbolId::TokenNewlineTimestamp);
    m_terminals.insert((uint32_t)SymbolId::TokenNewline);
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::add_rule(
        std::string const& name,
        std::unique_ptr<finite_automata::RegexAST<NFAStateType>> rule
) {
    Parser<NFAStateType, DFAStateType>::add_rule(name, std::move(rule));
    m_terminals.insert(this->m_lexer.m_symbol_id[name]);
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::add_token_group(
        std::string const& name,
        std::unique_ptr<finite_automata::RegexASTGroup<NFAStateType>> rule_group
) {
    add_rule(name, std::move(rule_group));
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::add_token_chain(
        std::string const& name,
        std::string const& chain
) {
    assert(chain.size() > 1);
    std::unique_ptr<finite_automata::RegexASTLiteral<NFAStateType>> first_char_rule
            = std::make_unique<finite_automata::RegexASTLiteral<NFAStateType>>(chain[0]);
    std::unique_ptr<finite_automata::RegexASTLiteral<NFAStateType>> second_char_rule
            = std::make_unique<finite_automata::RegexASTLiteral<NFAStateType>>(chain[1]);
    std::unique_ptr<finite_automata::RegexASTCat<NFAStateType>> rule_chain
            = std::make_unique<finite_automata::RegexASTCat<NFAStateType>>(
                    std::move(first_char_rule),
                    std::move(second_char_rule)
            );
    for (uint32_t i = 2; i < chain.size(); i++) {
        char next_char = chain[i];
        std::unique_ptr<finite_automata::RegexASTLiteral<NFAStateType>> next_char_rule
                = std::make_unique<finite_automata::RegexASTLiteral<NFAStateType>>(next_char);
        rule_chain = std::make_unique<finite_automata::RegexASTCat<NFAStateType>>(
                std::move(rule_chain),
                std::move(next_char_rule)
        );
    }
    add_rule(name, std::move(rule_chain));
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::add_production(
        std::string const& head,
        std::vector<std::string> const& body,
        SemanticRule semantic_rule
) -> uint32_t {
    if (this->m_lexer.m_symbol_id.find(head) == this->m_lexer.m_symbol_id.end()) {
        this->m_lexer.m_symbol_id[head] = this->m_lexer.m_symbol_id.size();
        this->m_lexer.m_id_symbol[this->m_lexer.m_symbol_id[head]] = head;
    }
    uint32_t n = m_productions.size();
    auto it = m_productions_map.find(head);
    if (it != m_productions_map.end()) {
        std::map<std::vector<std::string>, Production*>::iterator it2;
        it2 = it->second.find(body);
        if (it2 != it->second.end()) {
            it2->second->m_semantic_rule = semantic_rule;
            return n;
        }
    }
    std::unique_ptr<Production> p(new Production);
    p->m_index = n;
    p->m_head = this->m_lexer.m_symbol_id[head];
    for (std::string const& symbol_string : body) {
        if (this->m_lexer.m_symbol_id.find(symbol_string) == this->m_lexer.m_symbol_id.end()) {
            this->m_lexer.m_symbol_id[symbol_string] = this->m_lexer.m_symbol_id.size();
            this->m_lexer.m_id_symbol[this->m_lexer.m_symbol_id[symbol_string]] = symbol_string;
        }
        p->m_body.push_back(this->m_lexer.m_symbol_id[symbol_string]);
    }
    p->m_semantic_rule = std::move(semantic_rule);
    m_non_terminals.insert(std::pair<int, std::vector<Production*>>(p->m_head, {}));
    m_non_terminals[p->m_head].push_back(p.get());
    m_productions_map[head][body] = p.get();
    m_productions.push_back(std::move(p));
    if (m_productions.size() == 1) {
        m_root_production_id = add_production("$START_PRIME", {head}, nullptr);
    }
    return n;
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate() {
    this->m_lexer.generate();
    assert(!m_productions.empty());
    generate_lr0_kernels();
    generate_first_sets();
    generate_lr1_item_sets();
    generate_lalr1_parsing_table();
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lr0_kernels() {
    Production* root_production_ptr = m_productions[m_root_production_id].get();
    Item root_item(root_production_ptr, 0, cNullSymbol);
    std::unique_ptr<ItemSet> item_set0 = std::make_unique<ItemSet>();
    item_set0->m_kernel.insert(root_item);
    std::deque<ItemSet*> unused_item_sets;
    item_set0->m_index = m_lr0_item_sets.size();
    unused_item_sets.push_back(item_set0.get());
    m_lr0_item_sets[item_set0->m_kernel] = std::move(item_set0);
    while (!unused_item_sets.empty()) {
        ItemSet* item_set_ptr = unused_item_sets.back();
        unused_item_sets.pop_back();
        generate_lr0_closure(item_set_ptr);
        for (uint32_t const& next_symbol : m_terminals) {
            ItemSet* new_item_set_ptr = go_to(item_set_ptr, next_symbol);
            if (new_item_set_ptr != nullptr) {
                unused_item_sets.push_back(new_item_set_ptr);
            }
        }
        for (std::map<uint32_t, std::vector<Production*>>::value_type const& kv : m_non_terminals) {
            uint32_t next_symbol = kv.first;
            ItemSet* new_item_set_ptr = go_to(item_set_ptr, next_symbol);
            if (new_item_set_ptr != nullptr) {
                unused_item_sets.push_back(new_item_set_ptr);
            }
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::lr_closure_helper(
        ItemSet* item_set_ptr,
        Item const* item,
        uint32_t* next_symbol
) -> bool {
    // add {S'->(dot)S, ""}
    if (!item_set_ptr->m_closure.insert(*item).second) {
        return true;
    }
    if (item->has_dot_at_end()) {
        return true;
    }
    *next_symbol = item->next_symbol();
    if (this->symbol_is_token(*next_symbol)) {
        return true;
    }
    return false;
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lr0_closure(ItemSet* item_set_ptr) {
    std::deque<Item> q(
            item_set_ptr->m_kernel.begin(),
            item_set_ptr->m_kernel.end()
    );  // {{S'->(dot)S, ""}}
    while (!q.empty()) {
        Item item = q.back();  // {S'->(dot)S, ""}
        q.pop_back();
        uint32_t next_symbol = 0;
        if (lr_closure_helper(item_set_ptr, &item, &next_symbol)) {
            continue;
        }
        if (m_non_terminals.find(next_symbol) == m_non_terminals.end()) {
            assert(false);
        }
        for (Production* const p : m_non_terminals.at(next_symbol)) {
            // S -> a
            q.emplace_back(p, 0, cNullSymbol);  // {S -> (dot) a, ""}
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::go_to(
        ItemSet* from_item_set,
        uint32_t const& next_symbol
) -> ItemSet* {
    std::unique_ptr<ItemSet> next_item_set_ptr = std::make_unique<ItemSet>();
    assert(from_item_set != nullptr);
    for (Item const& item : from_item_set->m_closure) {
        if (item.has_dot_at_end()) {
            continue;
        }
        if (item.next_symbol() == next_symbol) {
            next_item_set_ptr->m_kernel
                    .emplace(item.m_production, item.m_dot + 1, item.m_lookahead);
        }
    }
    if (next_item_set_ptr->m_kernel.empty()) {
        return nullptr;
    }
    if (m_lr0_item_sets.find(next_item_set_ptr->m_kernel) != m_lr0_item_sets.end()) {
        ItemSet* existing_item_set_ptr = m_lr0_item_sets[next_item_set_ptr->m_kernel].get();
        m_go_to_table[from_item_set->m_index][next_symbol] = existing_item_set_ptr->m_index;
        from_item_set->m_next[next_symbol] = existing_item_set_ptr;
    } else {
        next_item_set_ptr->m_index = m_lr0_item_sets.size();
        m_go_to_table[from_item_set->m_index][next_symbol] = next_item_set_ptr->m_index;
        from_item_set->m_next[next_symbol] = next_item_set_ptr.get();
        m_lr0_item_sets[next_item_set_ptr->m_kernel] = std::move(next_item_set_ptr);
        return from_item_set->m_next[next_symbol];
    }
    return nullptr;
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_first_sets() {
    for (uint32_t const& s : m_terminals) {
        m_firsts.insert(std::pair<uint32_t, std::set<uint32_t>>(s, {s}));
    }
    bool changed = true;
    while (changed) {
        changed = false;
        for (std::unique_ptr<Production> const& p : m_productions) {
            std::set<uint32_t>& f = m_firsts[p->m_head];
            if (p->is_epsilon()) {
                changed = changed || m_nullable.insert(p->m_head).second;
                continue;
            }
            size_t old = f.size();
            size_t i = 0;
            for (uint32_t const& s : p->m_body) {
                std::set<uint32_t>& f2 = m_firsts[s];
                f.insert(f2.begin(), f2.end());
                if (m_nullable.find(s) == m_nullable.end()) {
                    break;
                }
                i++;
            }
            if (i == p->m_body.size()) {
                changed = changed || m_nullable.insert(p->m_head).second;
            }
            changed = changed || (f.size() != old);
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lr1_item_sets() {
    for (std::map<std::set<Item>, std::unique_ptr<ItemSet>>::value_type const& kv : m_lr0_item_sets)
    {
        for (Item const& l0_item : kv.second->m_kernel) {
            ItemSet temp_item_set;
            temp_item_set.m_kernel.insert(l0_item);
            generate_lr1_closure(&temp_item_set);
            for (Item const& l1_item : temp_item_set.m_closure) {
                if (l1_item.m_lookahead != cNullSymbol) {
                    m_spontaneous_map[l1_item.m_production].insert(l1_item.m_lookahead);
                } else {
                    if (l1_item.m_dot < l1_item.m_production->m_body.size()) {
                        Item temp_item(l1_item.m_production, l1_item.m_dot + 1, cNullSymbol);
                        m_propagate_map[l0_item].insert(temp_item);
                    }
                }
            }
        }
    }
    std::map<Item, std::set<int>> lookaheads;
    for (std::map<std::set<Item>, std::unique_ptr<ItemSet>>::value_type const& kv : m_lr0_item_sets)
    {
        for (Item const& l0_item : kv.second->m_kernel) {
            lookaheads[l0_item].insert(
                    m_spontaneous_map[l0_item.m_production].begin(),
                    m_spontaneous_map[l0_item.m_production].end()
            );
            if (l0_item.m_production == m_productions[m_root_production_id].get()) {
                lookaheads[l0_item].insert((uint32_t)SymbolId::TokenEnd);
            }
        }
    }
    bool changed = true;
    while (changed) {
        changed = false;
        for (std::map<Item, std::set<Item>>::value_type& kv : m_propagate_map) {
            Item item_from = kv.first;
            for (Item const& item_to : kv.second) {
                size_t size_before = lookaheads[item_to].size();
                lookaheads[item_to].insert(
                        lookaheads[item_from].begin(),
                        lookaheads[item_from].end()
                );
                size_t size_after = lookaheads[item_to].size();
                changed = changed || size_after > size_before;
            }
        }
    }
    for (std::map<std::set<Item>, std::unique_ptr<ItemSet>>::value_type const& kv : m_lr0_item_sets)
    {
        std::unique_ptr<ItemSet> lr1_item_set_ptr = std::make_unique<ItemSet>();
        for (Item const& l0_item : kv.second->m_kernel) {
            for (int const& lookahead : lookaheads[l0_item]) {
                Item lr1_item(l0_item.m_production, l0_item.m_dot, lookahead);
                lr1_item_set_ptr->m_kernel.insert(lr1_item);
            }
            if (l0_item.m_production == m_productions[m_root_production_id].get()
                && l0_item.m_dot == 0)
            {
                m_root_item_set_ptr = lr1_item_set_ptr.get();
            }
        }
        generate_lr1_closure(lr1_item_set_ptr.get());
        lr1_item_set_ptr->m_index = kv.second->m_index;
        m_lr1_item_sets[lr1_item_set_ptr->m_kernel] = std::move(lr1_item_set_ptr);
    }
    // this seems like the wrong way to do this still:
    for (std::map<std::set<Item>, std::unique_ptr<ItemSet>>::value_type const& kv1 :
         m_lr1_item_sets)
    {
        for (auto const& next_index : m_go_to_table[kv1.second->m_index]) {
            for (std::map<std::set<Item>, std::unique_ptr<ItemSet>>::value_type const& kv2 :
                 m_lr1_item_sets)
            {
                if (next_index.second == kv2.second->m_index) {
                    kv1.second->m_next[next_index.first] = kv2.second.get();
                    break;
                }
            }
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lr1_closure(ItemSet* item_set_ptr) {
    std::deque<Item> queue(item_set_ptr->m_kernel.begin(), item_set_ptr->m_kernel.end());
    while (!queue.empty()) {
        Item item = queue.back();
        queue.pop_back();
        uint32_t next_symbol = 0;
        if (lr_closure_helper(item_set_ptr, &item, &next_symbol)) {
            continue;
        }
        std::vector<uint32_t> lookaheads;
        size_t pos = item.m_dot + 1;
        while (pos < item.m_production->m_body.size()) {
            uint32_t symbol = item.m_production->m_body.at(pos);
            std::set<uint32_t> symbol_firsts = m_firsts.find(symbol)->second;
            lookaheads.insert(
                    lookaheads.end(),
                    std::make_move_iterator(symbol_firsts.begin()),
                    std::make_move_iterator(symbol_firsts.end())
            );
            if (m_nullable.find(symbol) == m_nullable.end()) {
                break;
            }
            pos++;
        }
        if (pos == item.m_production->m_body.size()) {
            lookaheads.push_back(item.m_lookahead);
        }
        for (Production* const p : m_non_terminals.at(next_symbol)) {
            for (uint32_t const& l : lookaheads) {
                queue.emplace_back(p, 0, l);
            }
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lalr1_parsing_table() {
    generate_lalr1_goto();
    generate_lalr1_action();
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lalr1_goto() {
    // done already at end of generate_lr1_item_sets()?
}

// Dragon book page 253
template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::generate_lalr1_action() {
    for (std::map<std::set<Item>, std::unique_ptr<ItemSet>>::value_type const& kv : m_lr1_item_sets)
    {
        ItemSet* item_set_ptr = kv.second.get();
        item_set_ptr->m_actions.resize(this->m_lexer.m_symbol_id.size(), false);
        for (Item const& item : item_set_ptr->m_closure) {
            if (!item.has_dot_at_end()) {
                if (m_terminals.find(item.next_symbol()) == m_terminals.end()
                    && m_non_terminals.find(item.next_symbol()) == m_non_terminals.end())
                {
                    continue;
                }
                assert(item_set_ptr->m_next.find(item.next_symbol()) != item_set_ptr->m_next.end());
                Action& action = item_set_ptr->m_actions[item.next_symbol()];
                if (!std::holds_alternative<bool>(action)) {
                    if (std::holds_alternative<ItemSet*>(action)
                        && std::get<ItemSet*>(action) == item_set_ptr->m_next[item.next_symbol()])
                    {
                        continue;
                    }
                    std::string conflict_msg{};
                    conflict_msg += "For symbol ";
                    conflict_msg += this->m_lexer.m_id_symbol[item.next_symbol()];
                    conflict_msg += ", adding shift to ";
                    conflict_msg
                            += std::to_string(item_set_ptr->m_next[item.next_symbol()]->m_index);
                    conflict_msg += " causes ";
                    if (std::holds_alternative<ItemSet*>(action)) {
                        conflict_msg += "shift-shift conflict with shift to ";
                        conflict_msg += std::to_string(std::get<ItemSet*>(action)->m_index);
                        conflict_msg += "\n";
                    } else {
                        conflict_msg += "shift-reduce conflict with reduction ";
                        conflict_msg
                                += this->m_lexer.m_id_symbol[std::get<Production*>(action)->m_head];
                        conflict_msg += "-> {";
                        for (uint32_t symbol : std::get<Production*>(action)->m_body) {
                            conflict_msg += this->m_lexer.m_id_symbol[symbol] + ",";
                        }
                        conflict_msg += "}\n";
                    }
                    throw std::runtime_error(conflict_msg);
                }
                item_set_ptr->m_actions[item.next_symbol()]
                        = item_set_ptr->m_next[item.next_symbol()];
            }
            if (item.has_dot_at_end()) {
                if (item.m_production == m_productions[m_root_production_id].get()) {
                    Action action = true;
                    item_set_ptr->m_actions[(uint32_t)SymbolId::TokenEnd] = action;
                } else {
                    Action& action = item_set_ptr->m_actions[item.m_lookahead];
                    if (!std::holds_alternative<bool>(action)) {
                        std::string conflict_msg{};
                        conflict_msg += "For symbol ";
                        conflict_msg += this->m_lexer.m_id_symbol[item.m_lookahead];
                        conflict_msg += ", adding reduction ";
                        conflict_msg += this->m_lexer.m_id_symbol[item.m_production->m_head];
                        conflict_msg += "-> {";
                        for (uint32_t symbol : item.m_production->m_body) {
                            conflict_msg += this->m_lexer.m_id_symbol[symbol] + ",";
                        }
                        conflict_msg += "} causes ";
                        if (std::holds_alternative<ItemSet*>(action)) {
                            conflict_msg += "shift-reduce conflict with shift to ";
                            conflict_msg += std::to_string(std::get<ItemSet*>(action)->m_index);
                            conflict_msg += "\n";
                        } else {
                            conflict_msg += "reduce-reduce conflict with reduction ";
                            conflict_msg
                                    += this->m_lexer
                                               .m_id_symbol[std::get<Production*>(action)->m_head];
                            conflict_msg += "-> {";
                            for (uint32_t symbol : std::get<Production*>(action)->m_body) {
                                conflict_msg += this->m_lexer.m_id_symbol[symbol] + ",";
                            }
                            conflict_msg += "}\n";
                        }
                        throw std::runtime_error(conflict_msg);
                    }
                    item_set_ptr->m_actions[item.m_lookahead] = item.m_production;
                }
            }
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::get_input_after_last_newline(
        std::stack<MatchedSymbol>& parse_stack_matches
) -> std::string {
    std::string error_message_reversed;
    bool done = false;
    while (!parse_stack_matches.empty() && !done) {
        MatchedSymbol top_symbol = std::move(parse_stack_matches.top());
        parse_stack_matches.pop();
        std::visit(
                Overloaded{
                        [&error_message_reversed, &done](Token& token) {
                            if (token.to_string() == "\r" || token.to_string() == "\n") {
                                done = true;
                            } else {
                                // input is being read backwards, so reverse
                                // each token so that when the entire input is
                                // reversed each token is displayed correctly
                                std::string token_string = token.to_string();
                                std::reverse(token_string.begin(), token_string.end());
                                error_message_reversed += token_string;
                            }
                        },
                        [&parse_stack_matches](NonTerminal& m) {
                            for (size_t i = 0; i < m.m_production->m_body.size(); i++) {
                                assert(m.m_children_start + i < cSizeOfAllChildren);
                                parse_stack_matches.push(std::move(
                                        NonTerminal::m_all_children[m.m_children_start + i]
                                ));
                            }
                        }
                },
                top_symbol
        );
    }
    std::reverse(error_message_reversed.begin(), error_message_reversed.end());
    return error_message_reversed;
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::get_input_until_next_newline(Token* error_token
) -> std::string {
    std::string rest_of_line;
    bool next_is_end_token = (error_token->m_type_ids_ptr->at(0) == (uint32_t)SymbolId::TokenEnd);
    bool next_has_newline = (error_token->to_string().find('\n') != std::string::npos)
                            || (error_token->to_string().find('\r') != std::string::npos);
    while (!next_has_newline && !next_is_end_token) {
        Token token = get_next_symbol();
        next_has_newline = (token.to_string().find('\n') != std::string::npos)
                           || (token.to_string().find('\r') != std::string::npos);
        if (!next_has_newline) {
            rest_of_line += token.to_string();
            next_is_end_token = (token.m_type_ids_ptr->at(0) == (uint32_t)SymbolId::TokenEnd);
        }
    }
    rest_of_line += "\n";
    return rest_of_line;
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::report_error() -> std::string {
    assert(m_next_token == std::nullopt);
    assert(!m_parse_stack_matches.empty());
    MatchedSymbol top_symbol = std::move(m_parse_stack_matches.top());
    m_parse_stack_matches.pop();
    uint32_t line_num = get_line_num(top_symbol);
    Token token = std::get<Token>(top_symbol);
    std::string consumed_input = get_input_after_last_newline(m_parse_stack_matches);
    std::string error_type{};
    std::string error_indicator;
    Token error_token = token;
    std::string rest_of_line = get_input_until_next_newline(&error_token);
    for (uint32_t i = 0; i < consumed_input.size() + 10; i++) {
        error_indicator += " ";
    }
    error_indicator += "^\n";
    if (token.m_type_ids_ptr->at(0) == (uint32_t)SymbolId::TokenEnd && consumed_input.empty()) {
        error_type = "empty file";
        error_indicator = "^\n";
    } else {
        error_type = "expected ";
        for (uint32_t i = 0; i < m_parse_stack_states.top()->m_actions.size(); i++) {
            Action action = m_parse_stack_states.top()->m_actions[i];
            if (action.index() != 0) {
                error_type += "'";
                if (auto* regex_ast_literal
                    = dynamic_cast<finite_automata::RegexASTLiteral<NFAStateType>*>(
                            this->m_lexer.get_rule(i)
                    ))
                {
                    error_type += unescape(char(regex_ast_literal->get_character()));
                } else {
                    error_type += this->m_lexer.m_id_symbol[i];
                }
                error_type += "',";
            }
        }
        error_type.pop_back();
        error_type += " before '" + unescape(token.to_string()[0]) + "' token";
    }
    std::string error_string = "Schema:" + std::to_string(line_num + 1) + ":"
                               + std::to_string(consumed_input.size() + 1)
                               + ": error: " + error_type + "\n";
    for (int i = 0; i < 10; i++) {
        error_string += " ";
    }
    error_string += consumed_input + error_token.to_string() + rest_of_line + error_indicator;
    return error_string;
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::parse(Reader& reader) -> NonTerminal {
    reset();
    m_parse_stack_states.push(m_root_item_set_ptr);
    bool accept = false;
    while (true) {
        m_input_buffer.read_if_safe(reader);
        Token next_terminal = get_next_symbol();
        if (parse_advance(next_terminal, &accept)) {
            break;
        }
    }
    if (!accept) {
        throw std::runtime_error(report_error());
    }
    assert(!m_parse_stack_matches.empty());
    MatchedSymbol m = std::move(m_parse_stack_matches.top());
    m_parse_stack_matches.pop();
    assert(m_parse_stack_matches.empty());
    return std::move(std::get<NonTerminal>(m));
}

template <typename NFAStateType, typename DFAStateType>
void LALR1Parser<NFAStateType, DFAStateType>::reset() {
    m_next_token = std::nullopt;
    while (!m_parse_stack_states.empty()) {
        m_parse_stack_states.pop();
    }
    while (!m_parse_stack_matches.empty()) {
        m_parse_stack_matches.pop();
    }
    m_input_buffer.reset();
    this->m_lexer.reset();
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::get_next_symbol() -> Token {
    if (m_next_token == std::nullopt) {
        Token token;
        if (ErrorCode error = this->m_lexer.scan(m_input_buffer, token);
            ErrorCode::Success != error)
        {
            throw std::runtime_error("Error scanning in lexer.");
        }
        return token;
    }
    Token s = m_next_token.value();
    m_next_token = std::nullopt;
    return s;
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::parse_advance(Token& next_token, bool* accept)
        -> bool {
    for (auto const type : *next_token.m_type_ids_ptr) {
        if (parse_symbol(type, next_token, accept)) {
            return *accept;
        }
    }
    assert(*accept == false);
    // For error handling
    m_parse_stack_matches.emplace(std::move(next_token));
    return true;
}

template <typename NFAStateType, typename DFAStateType>
auto LALR1Parser<NFAStateType, DFAStateType>::parse_symbol(
        uint32_t const& type_id,
        Token& next_token,
        bool* accept
) -> bool {
    ItemSet* curr = m_parse_stack_states.top();
    Action& it = curr->m_actions[type_id];
    bool ret = false;
    std::visit(
            Overloaded{
                    [&ret, &accept](bool is_accepting) {
                        if (!is_accepting) {
                            ret = false;
                            return;
                        }
                        *accept = true;
                        ret = true;
                        return;
                    },
                    [&ret, &next_token, this](ItemSet* shift) {
                        m_parse_stack_states.push(shift);
                        m_parse_stack_matches.emplace(std::move(next_token));
                        ret = true;
                        return;
                    },
                    [&ret, &next_token, this](Production* reduce) {
                        m_next_token = next_token;
                        NonTerminal matched_non_terminal(reduce);
                        size_t n = reduce->m_body.size();
                        for (size_t i = 0; i < n; i++) {
                            m_parse_stack_states.pop();
                            assert((matched_non_terminal.m_children_start + n - i - 1)
                                   < cSizeOfAllChildren);
                            NonTerminal::m_all_children
                                    [matched_non_terminal.m_children_start + n - i - 1]
                                    = std::move(m_parse_stack_matches.top());
                            m_parse_stack_matches.pop();
                        }
                        if (reduce->m_semantic_rule != nullptr) {
                            if (0 == m_next_token->m_start_pos) {
                                m_input_buffer.set_consumed_pos(
                                        m_input_buffer.storage().size() - 1
                                );
                            } else {
                                m_input_buffer.set_consumed_pos(m_next_token->m_start_pos - 1);
                            }
                            matched_non_terminal.m_ast
                                    = reduce->m_semantic_rule(&matched_non_terminal);
                        }
                        ItemSet* curr = m_parse_stack_states.top();
                        Action const& it
                                = curr->m_actions[matched_non_terminal.m_production->m_head];
                        m_parse_stack_states.push(std::get<ItemSet*>(it));
                        m_parse_stack_matches.emplace(std::move(matched_non_terminal));
                        ret = true;
                        return;
                    }
            },
            it
    );
    return ret;
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LALR1_PARSER_TPP
