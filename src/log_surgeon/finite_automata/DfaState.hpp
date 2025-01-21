#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE

#include <cassert>
#include <cstdint>
#include <memory>
#include <tuple>
#include <type_traits>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/SpontaneousTransition.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
template <StateType state_type>
class DfaState;

using ByteDfaState = DfaState<StateType::Byte>;
using Utf8DfaState = DfaState<StateType::Utf8>;
using OperationSequence = std::vector<TransitionOperation>;
using RegisterOperations = std::set<std::pair<register_id_t, OperationSequence>>;

template <StateType state_type>
class DfaState {
public:
    using Tree = UnicodeIntervalTree<DfaState*>;

    DfaState() {
        std::fill(
                std::begin(m_bytes_transition),
                std::end(m_bytes_transition),
                std::make_pair(RegisterOperations(), nullptr)
        );
    }

    auto add_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_ids.push_back(variable_id);
    }

    [[nodiscard]] auto get_matching_variable_ids() const -> std::vector<uint32_t> const& {
        return m_matching_variable_ids;
    }

    [[nodiscard]] auto is_accepting() const -> bool {
        return false == m_matching_variable_ids.empty();
    }

    auto add_byte_transition(uint8_t const& byte, RegisterOperations op_seq, DfaState* dest_state)
            -> void {
        m_bytes_transition[byte] = std::make_pair(op_seq, dest_state);
    }

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the DFA state on success.
     */
    [[nodiscard]] auto serialize(std::unordered_map<DfaState const*, uint32_t> const& state_ids
    ) const -> std::string;

    /**
     * @param character The character (byte or utf8) to transition on.
     * @return The register operations to perform, and a pointer to the DFA state reached, after
     * transitioning on `character`.
     */
    [[nodiscard]] auto next(uint32_t character) const -> std::pair<RegisterOperations, DfaState*>;

    [[nodiscard]] auto get_byte_transition(uint32_t character
    ) const -> std::pair<RegisterOperations, DfaState*> {
        return m_bytes_transition[character];
    }

private:
    std::vector<uint32_t> m_matching_variable_ids;
    std::pair<RegisterOperations, DfaState*> m_bytes_transition[cSizeOfByte];
    // NOTE: We don't need m_tree_transitions for the `state_type == StateType::Byte` case, so we
    // use an empty class (`std::tuple<>`) in that case.
    std::conditional_t<state_type == StateType::Utf8, Tree, std::tuple<>> m_tree_transitions;
};

template <StateType state_type>
auto DfaState<state_type>::next(uint32_t character
) const -> std::pair<RegisterOperations, DfaState*> {
    if constexpr (StateType::Byte == state_type) {
        return m_bytes_transition[character];
    } else {
        if (character < cSizeOfByte) {
            return m_bytes_transition[character];
        }
        std::unique_ptr<std::vector<typename Tree::Data>> result
                = m_tree_transitions.find(Interval(character, character));
        assert(result->size() <= 1);
        if (false == result->empty()) {
            return result->front().m_value;
        }
        return std::make_pair(RegisterOperations(), nullptr);
    }
}

template <StateType state_type>
auto DfaState<state_type>::serialize(std::unordered_map<DfaState const*, uint32_t> const& state_ids
) const -> std::string {
    auto const accepting_tags_string = is_accepting()
                                               ? fmt::format(
                                                         "accepting_tags={{{}}},",
                                                         fmt::join(m_matching_variable_ids, ",")
                                                 )
                                               : "";

    std::vector<std::string> byte_transitions;
    for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
        auto const [register_operations, dest_state]{m_bytes_transition[idx]};

        auto transformed_operations
                = register_operations
                  | std::ranges::views::transform(
                          [](std::pair<register_id_t, OperationSequence> const& reg_op) {
                              std::string op_string;
                              for (auto const op : reg_op.second) {
                                  if (TransitionOperation::SetTags == op) {
                                      op_string += "p";
                                  } else if (TransitionOperation::NegateTags == op) {
                                      op_string += "n";
                                  }
                              }
                              return fmt::format("{}{}", reg_op.first, op_string);
                          }
                  );

        if (nullptr != dest_state) {
            byte_transitions.emplace_back(fmt::format(
                    "{}-({})->{}",
                    static_cast<char>(idx),
                    fmt::join(transformed_operations, ","),
                    state_ids.at(dest_state)
            ));
        }
    }

    return fmt::format(
            "{}:{}byte_transitions={{{}}}",
            state_ids.at(this),
            accepting_tags_string,
            fmt::join(byte_transitions, ",")
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE
