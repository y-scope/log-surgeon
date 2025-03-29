#ifndef LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP

#include <map>
#include <optional>
#include <set>
#include <stack>
#include <stdexcept>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/TagOperation.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon::finite_automata {
/**
 * Encapsulates the configuration of a state during the determinization process of an NFA.
 *
 * This class stores the NFA state, the mapping of tag IDs to register IDs, the history of tag
 * operations, and the lookahead of upcoming tag operations.
 *
 * The configuration also supports exploring reachable configurations via spontaneous transitions.
 *
 * @tparam TypedNfaState The type of the NFA state.
 */
template <typename TypedNfaState>
class DeterminizationConfiguration {
public:
    DeterminizationConfiguration(
            TypedNfaState const* nfa_state,
            std::map<tag_id_t, reg_id_t> tag_to_reg_ids,
            std::vector<TagOperation> tag_history,
            std::vector<TagOperation> tag_lookahead
    )
            : m_nfa_state{nfa_state},
              m_tag_id_to_reg_ids{std::move(tag_to_reg_ids)},
              m_history{std::move(tag_history)},
              m_lookahead{std::move(tag_lookahead)} {
        if (nullptr == m_nfa_state) {
            throw std::invalid_argument("Determinization config cannot have a null NFA state.");
        }
    }

    auto operator<(DeterminizationConfiguration const& rhs) const -> bool {
        if (m_nfa_state->get_id() != rhs.m_nfa_state->get_id()) {
            return m_nfa_state->get_id() < rhs.m_nfa_state->get_id();
        }
        if (m_tag_id_to_reg_ids != rhs.m_tag_id_to_reg_ids) {
            return m_tag_id_to_reg_ids < rhs.m_tag_id_to_reg_ids;
        }
        if (m_history != rhs.m_history) {
            return m_history < rhs.m_history;
        }
        return m_lookahead < rhs.m_lookahead;
    }

    auto child_configuration_with_new_state(TypedNfaState const* new_nfa_state
    ) const -> DeterminizationConfiguration {
        return DeterminizationConfiguration(
                new_nfa_state,
                m_tag_id_to_reg_ids,
                m_history,
                m_lookahead
        );
    }

    [[nodiscard]] auto child_configuration_with_new_state_and_tag(
            TypedNfaState const* new_nfa_state,
            TagOperation const& tag_op
    ) const -> DeterminizationConfiguration {
        auto lookahead{m_lookahead};
        lookahead.push_back(tag_op);
        return DeterminizationConfiguration(
                new_nfa_state,
                m_tag_id_to_reg_ids,
                m_history,
                lookahead
        );
    }

    /**
     * @return The set of all configurations reachable from the current configuration via any number
     * of spontaneous transitions.
     */
    [[nodiscard]] auto spontaneous_closure() const -> std::set<DeterminizationConfiguration>;

    auto set_reg_id(tag_id_t const tag_id, reg_id_t const reg_id) -> void {
        m_tag_id_to_reg_ids.insert_or_assign(tag_id, reg_id);
    }

    [[nodiscard]] auto get_state() const -> TypedNfaState const* { return m_nfa_state; }

    [[nodiscard]] auto get_tag_id_to_reg_ids() const -> std::map<tag_id_t, reg_id_t> {
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

    [[nodiscard]] auto get_tag_lookahead(tag_id_t const tag_id
    ) const -> std::optional<TagOperation> {
        for (auto const tag_op : m_lookahead) {
            if (tag_op.get_tag_id() == tag_id) {
                return std::make_optional<TagOperation>(tag_op);
            }
        }
        return std::nullopt;
    }

private:
    /**
     * @param unexplored_stack Returns the stack of configurations updated to contain configurations
     * reachable from this configuration via a single spontaneous transition.
     */
    auto update_reachable_configs(std::stack<DeterminizationConfiguration>& unexplored_stack
    ) const -> void;

    TypedNfaState const* m_nfa_state;
    std::map<tag_id_t, reg_id_t> m_tag_id_to_reg_ids;
    std::vector<TagOperation> m_history;
    std::vector<TagOperation> m_lookahead;
};

template <typename TypedNfaState>
auto DeterminizationConfiguration<TypedNfaState>::update_reachable_configs(
        std::stack<DeterminizationConfiguration>& unexplored_stack
) const -> void {
    for (auto const& nfa_spontaneous_transition : m_nfa_state->get_spontaneous_transitions()) {
        auto child_config{this->child_configuration_with_new_state(
                nfa_spontaneous_transition.get_dest_state()
        )};
        for (auto const tag_op : nfa_spontaneous_transition.get_tag_ops()) {
            child_config = child_config.child_configuration_with_new_state_and_tag(
                    nfa_spontaneous_transition.get_dest_state(),
                    tag_op
            );
        }
        unexplored_stack.push(child_config);
    }
}

template <typename TypedNfaState>
auto DeterminizationConfiguration<TypedNfaState>::spontaneous_closure(
) const -> std::set<DeterminizationConfiguration> {
    std::set<DeterminizationConfiguration> reachable_set;
    std::stack<DeterminizationConfiguration> unexplored_stack;
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
