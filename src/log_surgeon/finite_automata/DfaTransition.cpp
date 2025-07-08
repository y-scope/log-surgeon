#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/DfaTransition.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>

#include <fmt/core.h>
#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <StateType state_type>
auto DfaTransition<state_type>::serialize(
        std::unordered_map<DfaState<state_type> const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    if (false == state_ids.contains(m_dest_state)) {
        return std::nullopt;
    }

    std::vector<std::string> transformed_ops;
    for (auto const& reg_op : m_reg_ops) {
        auto const optional_serialized_op{reg_op.serialize()};
        if (false == optional_serialized_op.has_value()) {
            return std::nullopt;
        }
        transformed_ops.emplace_back(optional_serialized_op.value());
    }

    return fmt::format("-({})->{}", fmt::join(transformed_ops, ","), state_ids.at(m_dest_state));
}

template auto DfaTransition<StateType::Byte>::serialize(
        std::unordered_map<ByteDfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;

template auto DfaTransition<StateType::Utf8>::serialize(
        std::unordered_map<Utf8DfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;
}  // namespace log_surgeon::finite_automata
