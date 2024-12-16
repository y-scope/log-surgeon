#ifndef LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP

#include <set>
#include <stack>
#include <utility>

#include <log_surgeon/finite_automata/Tag.hpp>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
class DetermizationConfiguration {
public:
    DetermizationConfiguration(
            TypedNfaState const* nfa_state,
            std::vector<uint32_t> register_ids,
            std::vector<Tag const*> history,
            std::vector<Tag const*> sequence
    )
            : m_nfa_state(nfa_state),
              m_register_ids(std::move(register_ids)),
              m_history(std::move(history)),
              m_sequence(std::move(sequence)) {}

    /**
     * Used for ordering in a set by considering the configuration's NFA state.
     * @param rhs The configuration to compare against.
     * @return Whether `m_nfa_state` in lhs has a lower address than in rhs.
     */
    auto operator<(DetermizationConfiguration const& rhs) const -> bool {
        return m_nfa_state < rhs.m_nfa_state;
    }

    auto configuration_with_new_state_and_tags(
            TypedNfaState const* new_nfa_state,
            std::vector<Tag const*> const& tags
    ) const -> DetermizationConfiguration {
        auto sequence{m_sequence};
        for (auto const* tag : tags) {
            sequence.push_back(tag);
        }
        return DetermizationConfiguration(new_nfa_state, m_register_ids, m_history, sequence);
    }

    /**
     * @param unexplored_stack Returns an updated stack of unexplored configurations that now
     * includes all configurations reachable from the current configuration via a single epsilon
     * transition.
     */
    auto epsilon_transition(std::stack<DetermizationConfiguration>& unexplored_stack) const -> void;

    /**
     * @return The set of all configurations reachable from the current configuration via any number
     * of epsilon transitions.
     */
    auto epsilon_closure() const -> std::set<DetermizationConfiguration>;

    [[nodiscard]] auto get_state() const -> TypedNfaState const* { return m_nfa_state; }

    [[nodiscard]] auto get_register_ids() const -> std::vector<uint32_t> { return m_register_ids; }

    [[nodiscard]] auto get_sequence() const -> std::vector<Tag const*> { return m_sequence; }

private:
    TypedNfaState const* m_nfa_state;
    std::vector<uint32_t> m_register_ids;
    std::vector<Tag const*> m_history;
    std::vector<Tag const*> m_sequence;
};

template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::epsilon_transition(
        std::stack<DetermizationConfiguration>& unexplored_stack
) const -> void {
    for (auto const* dest_state : m_nfa_state->get_epsilon_transitions()) {
        unexplored_stack.push(configuration_with_new_state_and_tags(dest_state, {}));
    }

    for (auto const& positive_tagged_start_transition :
         m_nfa_state->get_positive_tagged_start_transitions())
    {
        auto const* dest_state = positive_tagged_start_transition.get_dest_state();
        auto const* tag = positive_tagged_start_transition.get_tag();
        unexplored_stack.push(configuration_with_new_state_and_tags(dest_state, {tag}));
    }

    auto const& optional_positive_tagged_end_transition
            = m_nfa_state->get_positive_tagged_end_transition();
    if (optional_positive_tagged_end_transition.has_value()) {
        auto const* dest_state{optional_positive_tagged_end_transition.value().get_dest_state()};
        auto const* tag{optional_positive_tagged_end_transition.value().get_tag()};
        unexplored_stack.push(configuration_with_new_state_and_tags(dest_state, {tag}));
    }

    auto const& optional_negative_tagged_transition = m_nfa_state->get_negative_tagged_transition();
    if (optional_negative_tagged_transition.has_value()) {
        auto const* dest_state = optional_negative_tagged_transition.value().get_dest_state();
        auto tags = optional_negative_tagged_transition.value().get_tags();
        unexplored_stack.push(configuration_with_new_state_and_tags(dest_state, tags));
    }
}

template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::epsilon_closure(
) const -> std::set<DetermizationConfiguration> {
    std::set<DetermizationConfiguration> reachable_set;
    std::stack<DetermizationConfiguration> unexplored_stack;
    unexplored_stack.push(*this);
    while (false == unexplored_stack.empty()) {
        auto const& current_configuration = unexplored_stack.top();
        unexplored_stack.pop();

        if (false == reachable_set.insert(current_configuration).second) {
            continue;
        }

        current_configuration.epsilon_transition(unexplored_stack);
    }
    return reachable_set;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
