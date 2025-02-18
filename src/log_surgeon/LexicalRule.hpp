#ifndef LOG_SURGEON_LEXICAL_RULE_HPP
#define LOG_SURGEON_LEXICAL_RULE_HPP
#include <cstdint>
#include <memory>
#include <vector>

#include <log_surgeon/finite_automata/Capture.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>

namespace log_surgeon {
template <typename TypedNfaState>
class LexicalRule {
public:
    // Constructor
    LexicalRule(
            uint32_t const variable_id,
            std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> regex
    )
            : m_variable_id(variable_id),
              m_regex(std::move(regex)) {}

    /**
     * Adds AST representing the lexical rule to the NFA
     * @param nfa
     */
    auto add_to_nfa(finite_automata::Nfa<TypedNfaState>* nfa) const -> void;

    [[nodiscard]] auto get_captures() const -> std::vector<finite_automata::Capture const*> const& {
        return m_regex->get_subtree_positive_captures();
    }

    [[nodiscard]] auto get_variable_id() const -> uint32_t { return m_variable_id; }

    [[nodiscard]] auto get_regex() const -> finite_automata::RegexAST<TypedNfaState>* {
        // TODO: make the returned pointer constant
        return m_regex.get();
    }

private:
    uint32_t m_variable_id;
    std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> m_regex;
};

template <typename TypedNfaState>
void LexicalRule<TypedNfaState>::add_to_nfa(finite_automata::Nfa<TypedNfaState>* nfa) const {
    auto* end_state = nfa->new_accepting_state(m_variable_id);
    m_regex->add_to_nfa_with_negative_captures(nfa, end_state);
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LEXICAL_RULE_HPP
