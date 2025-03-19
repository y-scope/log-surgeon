#ifndef LOG_SURGEON_PRODUCTION_HPP
#define LOG_SURGEON_PRODUCTION_HPP

#include <cstdint>
#include <vector>

#include <log_surgeon/types.hpp>

namespace log_surgeon {
/**
 * Structure representing a production of the form "m_head -> {m_body}". The code fragment to
 * execute upon reducing "{m_body} -> m_head" is m_semantic_rule, which is purely a function of the
 * MatchedSymbols for {m_body}. m_index is the productions position in the parsers production
 * vector.
 */
class Production {
public:
    /**
     * Returns if the production is an epsilon production. An epsilon production has nothing on its
     * LHS (i.e., HEAD -> {})
     * @return bool
     */
    [[nodiscard]] auto is_epsilon() const -> bool { return m_body.empty(); }

    uint32_t m_index;
    uint32_t m_head;
    std::vector<uint32_t> m_body;
    SemanticRule m_semantic_rule;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_PRODUCTION_HPP
