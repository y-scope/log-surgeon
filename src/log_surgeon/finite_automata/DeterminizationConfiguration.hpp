#ifndef LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP

#include <map>
#include <optional>
#include <set>
#include <stack>
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
            std::map<tag_id_t, register_id_t> tag_to_reg_ids,
            std::vector<TagOperation> tag_history,
            std::vector<TagOperation> tag_lookahead
    )
            : m_nfa_state(nfa_state),
              m_tag_id_to_reg_ids(std::move(tag_to_reg_ids)),
              m_history(std::move(tag_history)),
              m_lookahead(std::move(tag_lookahead)) {}

    auto operator<(DetermizationConfiguration const& rhs) const -> bool {
        if (m_nfa_state != rhs.m_nfa_state) {
            return m_nfa_state < rhs.m_nfa_state;
        }
        if (m_tag_id_to_reg_ids != rhs.m_tag_id_to_reg_ids) {
            return m_tag_id_to_reg_ids < rhs.m_tag_id_to_reg_ids;
        }
        if (m_history != rhs.m_history) {
            return m_history < rhs.m_history;
        }
        return m_lookahead < rhs.m_lookahead;
    }

    auto child_configuration_with_new_state_and_tag(
            TypedNfaState const* new_nfa_state,
            TagOperation const& tag_op
    ) const -> DetermizationConfiguration {
        auto lookahead{m_lookahead};
        lookahead.push_back(tag_op);
        return DetermizationConfiguration(new_nfa_state, m_tag_id_to_reg_ids, m_history, lookahead);
    }

    /**
     * @unexplored_stack Returns the stack of configurations updated to contain configurations
     * reachable from this configuration via a single spontaneous transition.
     */
    auto update_reachable_configs(std::stack<DetermizationConfiguration>& unexplored_stack
    ) const -> void;

    /**
     * @return The set of all configurations reachable from the current configuration via any number
     * of spontaneous transitions.
     */
    auto spontaneous_closure() const -> std::set<DetermizationConfiguration>;

    auto set_reg_id(tag_id_t const tag_id, register_id_t const reg_id) {
        m_tag_id_to_reg_ids[tag_id] = reg_id;
    }

    [[nodiscard]] auto get_state() const -> TypedNfaState const* { return m_nfa_state; }

    [[nodiscard]] auto get_tag_id_to_reg_ids() const -> std::map<tag_id_t, register_id_t> {
        return m_tag_id_to_reg_ids;
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

    [[nodiscard]] auto get_tag_lookahead(tag_id_t const tag_id) const -> std::optional<TagOperation> {
        for (auto const tag_op : m_lookahead) {
            if (tag_op.get_tag_id() == tag_id) {
                return std::make_optional<TagOperation>(tag_op);
            }
        }
        return std::nullopt;
    }

private:
    TypedNfaState const* m_nfa_state;
    std::map<tag_id_t, register_id_t> m_tag_id_to_reg_ids;
    std::vector<TagOperation> m_history;
    std::vector<TagOperation> m_lookahead;
};

template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::update_reachable_configs(
        std::stack<DetermizationConfiguration>& unexplored_stack
) const -> void {
    for (auto const& nfa_spontaneous_transition : m_nfa_state->get_spontaneous_transitions()) {
        auto parent_config{*this};
        for (auto const tag_op : nfa_spontaneous_transition.get_tag_ops()) {
            auto child_config{parent_config.child_configuration_with_new_state_and_tag(
                    nfa_spontaneous_transition.get_dest_state(),
                    tag_op
            )};
            parent_config = child_config;
        }
        unexplored_stack.push(parent_config);
    }
}

template <typename TypedNfaState>
auto DetermizationConfiguration<TypedNfaState>::spontaneous_closure(
) const -> std::set<DetermizationConfiguration> {
    std::set<DetermizationConfiguration> reachable_set;
    std::stack<DetermizationConfiguration> unexplored_stack;
    unexplored_stack.push(*this);
    while (false == unexplored_stack.empty()) {
        auto current_configuration = unexplored_stack.top();
        unexplored_stack.pop();
        if (false == reachable_set.contains(current_configuration)) {
            current_configuration.update_reachable_configs(unexplored_stack);
            reachable_set.insert(current_configuration);
        }
    }
    return reachable_set;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
