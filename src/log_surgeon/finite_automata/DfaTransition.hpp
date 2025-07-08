#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFATRANSITION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DFATRANSITION_HPP

#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/RegisterOperation.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>

namespace log_surgeon::finite_automata {
template <StateType state_type>
class DfaState;

/**
 * Represents a transition in a DFA. A transition consists of:
 * - A destination state.
 * - A set of register operations to be performed when the transition is taken.
 *
 * @tparam state_type Specifies the type of transition (bytes or UTF-8 characters).
 */
template <StateType state_type>
class DfaTransition {
public:
    DfaTransition(std::vector<RegisterOperation> reg_ops, DfaState<state_type> const* dest_state)
            : m_reg_ops{std::move(reg_ops)},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_reg_ops() const -> std::vector<RegisterOperation> const& {
        return m_reg_ops;
    }

    [[nodiscard]] auto get_dest_state() const -> DfaState<state_type> const* {
        return m_dest_state;
    }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the DFA transition on success.
     * @return Forwards `RegisterOperation::serialize`'s return value (std::nullopt) on failure.
     * @return std::nullopt if `m_dest_state` is not in `state_ids`.
     */
    [[nodiscard]] auto serialize(
            std::unordered_map<DfaState<state_type> const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    std::vector<RegisterOperation> m_reg_ops;
    DfaState<state_type> const* m_dest_state{nullptr};
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFATRANSITION_HPP
