#ifndef LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP

#include <optional>
#include <set>
#include <stack>
#include <unordered_map>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/Register.hpp>
#include <log_surgeon/finite_automata/TagOperation.hpp>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
class DetermizationConfiguration {
public:
    DetermizationConfiguration(
            TypedNfaState const* nfa_state,
            std::unordered_map<tag_id_t, register_id_t> tag_to_reg_ids,
            std::vector<TagOperation> tag_history,
            std::vector<TagOperation> tag_lookahead
    )
            : m_nfa_state(nfa_state),
              m_tag_to_reg_ids(std::move(tag_to_reg_ids)),
              m_history(std::move(tag_history)),
              m_lookahead(std::move(tag_lookahead)) {}

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
            TagOperation const& tag_op
    ) const -> DetermizationConfiguration {
        auto lookahead{m_lookahead};
        lookahead.push_back(tag_op);
        return DetermizationConfiguration(new_nfa_state, m_tag_to_reg_ids, m_history, lookahead);
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

    [[nodiscard]] auto get_tag_history(tag_id_t const tag_id) const -> std::optional<TagOperation> {
        for (auto const tag_op : m_history) {
            if (tag_op.get_tag_id() == tag_id) {
                return std::make_optional<TagOperation>(tag_op);
            }
        }
        return std::nullopt;
    }

    [[nodiscard]] auto get_lookahead() const -> std::vector<TagOperation> { return m_lookahead; }

private:
    TypedNfaState const* m_nfa_state;
    std::unordered_map<tag_id_t, register_id_t> m_tag_to_reg_ids;
    std::vector<TagOperation> m_history;
    std::vector<TagOperation> m_lookahead;
};

// TODO: change the name and return the new elements
template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::spontaneous_transition(
        std::stack<DetermizationConfiguration>& unexplored_stack
) const -> void {
    for (auto const& nfa_spontaneous_transition : m_nfa_state->get_spontaneous_transitions()) {
        // TODO: this approach is confusing, change it to something more intuitive.
        unexplored_stack.push(*this);
        for (auto const tag_op : nfa_spontaneous_transition.get_tag_ops()) {
            auto parent_config{unexplored_stack.top()};
            unexplored_stack.pop();

            auto child_config{parent_config.child_configuration_with_new_state_and_tag(
                    nfa_spontaneous_transition.get_dest_state(),
                    tag_op
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

        auto const inserted_configuration = reachable_set.insert(current_configuration);
        if (inserted_configuration.second) {
            inserted_configuration.first->spontaneous_transition(unexplored_stack);
        }
    }
    return reachable_set;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
