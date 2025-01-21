#ifndef LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP

#include <set>
#include <stack>
#include <utility>

#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/Register.hpp>

namespace log_surgeon::finite_automata {
using OpTagPair = std::pair<TransitionOperation, tag_id_t>;
using TagSequence = std::vector<OpTagPair>;

template <typename TypedNfaState>
class   DetermizationConfiguration {
public:
    DetermizationConfiguration(
            TypedNfaState const* nfa_state,
            std::unordered_map<tag_id_t, register_id_t> tag_to_reg_ids,
            TagSequence tag_history,
            TagSequence tag_sequence
    )
            : m_nfa_state(nfa_state),
              m_tag_to_reg_ids(std::move(tag_to_reg_ids)),
              m_tag_history(std::move(tag_history)),
              m_tag_sequence(std::move(tag_sequence)) {}

    /**
     * Used for ordering in a set by considering the configuration's NFA state.
     * @param rhs The configuration to compare against.
     * @return Whether `m_nfa_state` in lhs has a lower address than in rhs.
     */
    auto operator<(DetermizationConfiguration const& rhs) const -> bool {
        return m_nfa_state < rhs.m_nfa_state;
    }

    auto child_configuration_with_new_state_and_tag(
            TypedNfaState const* new_nfa_state,
            OpTagPair const& op_tag_pair
    ) const -> DetermizationConfiguration {
        auto sequence{m_tag_sequence};
        sequence.push_back(op_tag_pair);
        return DetermizationConfiguration(new_nfa_state, m_tag_to_reg_ids, m_tag_history, sequence);
    }

    /**
     * @param unexplored_stack Returns an updated stack of unexplored configurations that now
     * includes all configurations reachable from the current configuration via a single epsilon
     * transition.
     */
    auto spontaneous_transition(std::stack<DetermizationConfiguration>& unexplored_stack
    ) const -> void;

    /**
     * @return The set of all configurations reachable from the current configuration via any number
     * of epsilon transitions.
     */
    auto spontaneous_closure() const -> std::set<DetermizationConfiguration>;

    auto set_reg_id(tag_id_t const tag_id, register_id_t const reg_id) {
        m_tag_to_reg_ids[tag_id] = reg_id;
    }

    [[nodiscard]] auto get_state() const -> TypedNfaState const* { return m_nfa_state; }

    [[nodiscard]] auto get_tag_to_reg_ids() const -> std::unordered_map<tag_id_t, register_id_t> {
        return m_tag_to_reg_ids;
    }

    [[nodiscard]] auto get_tag_history(tag_id_t const tag_id) const -> OperationSequence {
        auto tag_history{m_tag_history};
        return get_tag_history_helper(tag_id, tag_history);
    }

    [[nodiscard]] auto get_sequence() const -> TagSequence { return m_tag_sequence; }

private:
    [[nodiscard]] static auto
    get_tag_history_helper(tag_id_t const tag_id, TagSequence& history) -> OperationSequence {
        OperationSequence tag_history;
        if (history.empty()) {
            return tag_history;
        }

        if (tag_id == history.back().second) {
            tag_history.push_back(history.back().first);
        }
        history.pop_back();
        auto recursive_tag_history{get_tag_history_helper(tag_id, history)};
        tag_history.insert(
                tag_history.begin(),
                recursive_tag_history.begin(),
                recursive_tag_history.end()
        );
        return tag_history;
    }

    TypedNfaState const* m_nfa_state;
    std::unordered_map<tag_id_t, register_id_t> m_tag_to_reg_ids;
    TagSequence m_tag_history;
    TagSequence m_tag_sequence;
};

template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::spontaneous_transition(
        std::stack<DetermizationConfiguration>& unexplored_stack
) const -> void {
    for (auto const& nfa_spontaneous_transition : m_nfa_state->get_spontaneous_transitions()) {
        unexplored_stack.push(*this);
        for (auto const tag_id : nfa_spontaneous_transition.get_tag_ids()) {
            auto parent_config{unexplored_stack.top()};
            unexplored_stack.pop();

            auto op_tag_pair{
                    std::make_pair(nfa_spontaneous_transition.get_transition_operation(), tag_id)
            };
            auto child_config{parent_config.child_configuration_with_new_state_and_tag(
                    nfa_spontaneous_transition.get_dest_state(),
                    op_tag_pair
            )};
            unexplored_stack.push(child_config);
        }
    }
}

template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::spontaneous_closure(
) const -> std::set<DetermizationConfiguration> {
    std::set<DetermizationConfiguration> reachable_set;
    std::stack<DetermizationConfiguration> unexplored_stack;
    unexplored_stack.push(*this);
    while (false == unexplored_stack.empty()) {
        auto const current_configuration = unexplored_stack.top();
        unexplored_stack.pop();

        auto inserted_configuration = reachable_set.insert(current_configuration);
        if (inserted_configuration.second) {
            inserted_configuration.first->spontaneous_transition(unexplored_stack);
        }
    }
    return reachable_set;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
