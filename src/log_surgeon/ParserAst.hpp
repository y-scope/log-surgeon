#ifndef LOG_SURGEON_PARSERAST_HPP
#define LOG_SURGEON_PARSERAST_HPP

#include <utility>

namespace log_surgeon {
template <typename T>
class ParserValue;

class ParserAST {
public:
    virtual ~ParserAST() = 0;

    template <typename T>
    auto get() -> T& {
        return static_cast<ParserValue<T>*>(this)->m_value;
    }
};

template <typename T>
class ParserValue : public ParserAST {
public:
    T m_value;

    explicit ParserValue(T val) : m_value(std::move(val)) {}
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_PARSERAST_HPP
