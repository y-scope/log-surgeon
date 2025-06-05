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
 * Represents a single configuration used during the tagged determinization from NFA to DFA.
 *
 * A configuration captures a snapshot of the NFA's execution, including:
 * - the current NFA state,
 * - a mapping from tag IDs to register IDs,
 * - the history of tag operations,
 * - and the lookahead for upcoming tag operations.
 *
 * During determinization, sets of configurations are grouped to form DFA states—similar to how
 * sets of NFA states form DFA states in classical (untagged) subset construction. However, unlike
 * untagged determinization, configurations preserve tag histories, allowing the tagged determinizer
 * to distinguish between paths that share the same NFA state but differ in how tags are applied.
 *
 * This distinction is essential for building a tagged DFA (TDFA) from a tagged NFA (TNFA), as it
 * preserves paths in the NFA that match the same untagged regex but have unique tag positions.
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
            std::map<tag_id_t, reg_id_t> tag_id_to_reg_ids,
            std::vector<TagOperation> tag_history,
            std::vector<TagOperation> tag_lookahead
    )
            : m_nfa_state{nfa_state},
              m_tag_id_to_reg_ids{std::move(tag_id_to_reg_ids)},
              m_history{std::move(tag_history)},
              m_lookahead{std::move(tag_lookahead)} {
        if (nullptr == m_nfa_state) {
            throw std::invalid_argument("Determinization config cannot have a null NFA state.");
        }
    }

    /**
     * Compares this configuration with another to establish a strict weak ordering.
     *
     * This operator is used to insert configurations into ordered containers such as `std::set`
     * or `std::map`. The comparison considers:
     * 1. The NFA state ID.
     * 2. The mapping of tag IDs to register IDs.
     * 3. The history of tag operations.
     * 4. The lookahead for upcoming tag operations.
     *
     * The ordering ensures that configurations with the same NFA state but different tag histories
     * or register mappings are treated as distinct.
     *
     * @param rhs The configuration to compare against.
     * @return `true` if this configuration is considered less than `rhs`, `false` otherwise.
     */
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

    auto child_configuration_with_new_state(TypedNfaState const* new_nfa_state) const
            -> DeterminizationConfiguration {
        return DeterminizationConfiguration(
                new_nfa_state,
                m_tag_id_to_reg_ids,
                m_history,
                m_lookahead
        );
    }

    /**
     * Creates a new configuration from the current configuration by replacing the NFA state and
     * appending a future tag operation.
     *
     * This is used during determinization to create configurations during the closure.
     *
     * @param new_nfa_state The NFA state to use in the new configuration.
     * @param tag_op The tag operation to append to the lookahead.
     * @return A new `DeterminizationConfiguration` with the updated state and lookahead.
     */
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
     * @return The set of all configurations reachable from this configuration via any number
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

    [[nodiscard]] auto get_tag_lookahead(tag_id_t const tag_id) const
            -> std::optional<TagOperation> {
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
    auto update_reachable_configs(std::stack<DeterminizationConfiguration>& unexplored_stack) const
            -> void;

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
auto DeterminizationConfiguration<TypedNfaState>::spontaneous_closure() const
        -> std::set<DeterminizationConfiguration> {
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
