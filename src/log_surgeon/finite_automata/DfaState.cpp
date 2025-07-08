#include "DfaState.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/DfaTransition.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>

#include <fmt/core.h>
#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <StateType state_type>
auto DfaState<state_type>::serialize(
        std::unordered_map<DfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto const accepting_tags_string{
            is_accepting()
                    ? fmt::format("accepting_tags={{{}}},", fmt::join(m_matching_variable_ids, ","))
                    : ""
    };

    std::vector<std::string> accepting_op_strings;
    for (auto const& accepting_op : m_accepting_ops) {
        auto const serialized_accepting_op{accepting_op.serialize()};
        if (serialized_accepting_op.has_value()) {
            accepting_op_strings.push_back(serialized_accepting_op.value());
        }
    }
    auto const accepting_ops_string{
            is_accepting() ? fmt::format(
                                     "accepting_operations={{{}}},",
                                     fmt::join(accepting_op_strings, ",")
                             )
                           : ""
    };

    std::vector<std::string> transition_strings;
    for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
        if (false == m_bytes_transition[idx].has_value()) {
            continue;
        }
        auto const optional_byte_transition_string{m_bytes_transition[idx]->serialize(state_ids)};
        if (false == optional_byte_transition_string.has_value()) {
            return std::nullopt;
        }
        transition_strings.emplace_back(
                fmt::format("{}{}", static_cast<char>(idx), optional_byte_transition_string.value())
        );
    }

    return fmt::format(
            "{}:{}{}byte_transitions={{{}}}",
            state_ids.at(this),
            accepting_tags_string,
            accepting_ops_string,
            fmt::join(transition_strings, ",")
    );
}

template auto ByteDfaState::serialize(
        std::unordered_map<ByteDfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;

template auto Utf8DfaState::serialize(
        std::unordered_map<Utf8DfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;

template <>
auto ByteDfaState::get_transition(uint8_t const character) const
        -> std::optional<DfaTransition<StateType::Byte>> const& {
    return m_bytes_transition[character];
}

template <>
auto Utf8DfaState::get_transition(uint8_t const character) const
        -> std::optional<DfaTransition<StateType::Utf8>> const& {
    // TODO: Handle UTF8
    return m_bytes_transition[character];
}
}  // namespace log_surgeon::finite_automata
