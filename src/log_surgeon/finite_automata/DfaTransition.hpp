#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFATRANSITION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DFATRANSITION_HPP

#include <cstdint>
#include <optional>
#include <ranges>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <fmt/core.h>
#include <fmt/format.h>

#include <log_surgeon/finite_automata/RegisterOperation.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>

namespace log_surgeon::finite_automata {
template <StateType state_type>
class DfaState;

template <StateType state_type>
class DfaTransition {
public:
    DfaTransition() = default;

    DfaTransition(std::vector<RegisterOperation> reg_ops, DfaState<state_type>* dest_state)
            : m_reg_ops{std::move(reg_ops)},
              m_dest_state{dest_state} {}

    [[nodiscard]] auto get_reg_ops() const -> std::vector<RegisterOperation> { return m_reg_ops; }

    [[nodiscard]] auto get_dest_state() const -> DfaState<state_type>* { return m_dest_state; }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the DFA transition on success.
     * @return std::nullopt on failure.
     */
    [[nodiscard]] auto serialize(
            std::unordered_map<DfaState<state_type> const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    std::vector<RegisterOperation> m_reg_ops;
    DfaState<state_type>* m_dest_state{nullptr};
};

template <StateType state_type>
auto DfaTransition<state_type>::serialize(
        std::unordered_map<DfaState<state_type> const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto transformed_operations
            = m_reg_ops | std::ranges::views::transform([](RegisterOperation const& reg_op) {
                  return reg_op.serialize();
              });

    if (false == state_ids.contains(m_dest_state)) {
        return std::nullopt;
    }

    return fmt::format(
            "-({})->{}",
            fmt::join(transformed_operations, ","),
            state_ids.at(m_dest_state)
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFATRANSITION_HPP
