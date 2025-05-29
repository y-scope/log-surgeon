#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE

#include <array>
#include <cassert>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <tuple>
#include <type_traits>
#include <unordered_map>
#include <vector>

#include <fmt/core.h>
#include <fmt/format.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/DfaTransition.hpp>
#include <log_surgeon/finite_automata/RegisterOperation.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
template <StateType state_type>
class DfaState;

using ByteDfaState = DfaState<StateType::Byte>;
using Utf8DfaState = DfaState<StateType::Utf8>;

template <StateType state_type>
class DfaState {
public:
    using Tree = UnicodeIntervalTree<DfaState*>;

    DfaState() = default;

    auto add_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_ids.push_back(variable_id);
    }

    [[nodiscard]] auto get_matching_variable_ids() const -> std::vector<uint32_t> const& {
        return m_matching_variable_ids;
    }

    [[nodiscard]] auto is_accepting() const -> bool {
        return false == m_matching_variable_ids.empty();
    }

    auto add_byte_transition(uint8_t const byte, DfaTransition<state_type> dfa_transition) -> void {
        m_bytes_transition[byte] = dfa_transition;
    }

    auto add_accepting_op(RegisterOperation const reg_op) -> void {
        m_accepting_ops.push_back(reg_op);
    }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the DFA state.
     * @return Forwards `DfaTransition::serialize`'s return value (std::nullopt) on failure.
     */
    [[nodiscard]] auto serialize(std::unordered_map<DfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

    /**
     * @param character The character (byte or utf8) to transition on.
     * @return The transition, which contains the register operations and destination state.
     */
    [[nodiscard]] auto get_transition(uint8_t character
    ) const -> std::optional<DfaTransition<state_type>> const&;

    [[nodiscard]] auto get_accepting_reg_ops() const -> std::vector<RegisterOperation> const& {
        return m_accepting_ops;
    }

private:
    std::vector<uint32_t> m_matching_variable_ids;
    std::vector<RegisterOperation> m_accepting_ops;
    std::array<std::optional<DfaTransition<state_type>>, cSizeOfByte> m_bytes_transition;
    // NOTE: We don't need m_tree_transitions for the `state_type == StateType::Byte` case, so we
    // use an empty class (`std::tuple<>`) in that case.
    std::conditional_t<state_type == StateType::Utf8, Tree, std::tuple<>> m_tree_transitions;
};

// TODO: Handle UTF-8.
template <>
[[nodiscard]] inline auto DfaState<StateType::Byte>::get_transition(uint8_t const character
) const -> std::optional<DfaTransition<StateType::Byte>> const& {
    return m_bytes_transition[character];
}

template <StateType state_type>
auto DfaState<state_type>::serialize(std::unordered_map<DfaState const*, uint32_t> const& state_ids
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
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE
