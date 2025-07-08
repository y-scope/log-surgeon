#include "NfaState.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>

#include <fmt/core.h>
#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <StateType state_type>
auto NfaState<state_type>::serialize(
        std::unordered_map<NfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto const accepting_tag_string{
            m_accepting ? fmt::format("accepting_tag={},", m_matching_variable_id) : ""
    };

    std::vector<std::string> byte_transitions;
    for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
        for (auto const* dest_state : m_bytes_transitions.at(idx)) {
            byte_transitions.emplace_back(
                    fmt::format("{}-->{}", static_cast<char>(idx), state_ids.at(dest_state))
            );
        }
    }

    std::vector<std::string> serialized_spontaneous_transitions;
    for (auto const& spontaneous_transition : m_spontaneous_transitions) {
        auto const optional_serialized_transition{spontaneous_transition.serialize(state_ids)};
        if (false == optional_serialized_transition.has_value()) {
            return std::nullopt;
        }
        serialized_spontaneous_transitions.emplace_back(optional_serialized_transition.value());
    }

    return fmt::format(
            "{}:{}byte_transitions={{{}}},spontaneous_transition={{{}}}",
            state_ids.at(this),
            accepting_tag_string,
            fmt::join(byte_transitions, ","),
            fmt::join(serialized_spontaneous_transitions, ",")
    );
}

template auto NfaState<StateType::Byte>::serialize(
        std::unordered_map<ByteNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;
}  // namespace log_surgeon::finite_automata
