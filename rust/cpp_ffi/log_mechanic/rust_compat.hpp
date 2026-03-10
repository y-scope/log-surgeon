#ifndef LOGMECH_RUST_COMPAT_HPP
#define LOGMECH_RUST_COMPAT_HPP

#include <cstddef>
#include <string_view>
#include <type_traits>

namespace log_mechanic {
template <typename T>
using Box = T*;

template <typename T>
struct is_rust_box_t : std::false_type {}; // NOLINT(readability-identifier-naming)

template <typename T>
struct is_rust_box_t<Box<T>> : std::true_type {};

template <typename T>
constexpr bool is_rust_box_v = is_rust_box_t<T>::value; // NOLINT(readability-identifier-naming)

template <typename T>
using Option = std::enable_if_t<is_rust_box_v<T>, T>;

template <typename T>
struct CArray {
    T const* pointer;
    size_t length;

    [[nodiscard]] static auto from_ptr_len(T const* ptr, size_t len) noexcept -> CArray {
        return CArray { ptr, len };
    }

    [[nodiscard]] static auto from_string_view(std::string_view const& view) noexcept -> CArray
    requires std::is_same_v<T, char>
    {
        return CArray::from_ptr_len(view.data(), view.length());
    }

    [[nodiscard]] static auto from_c_str(char const* c_str) noexcept -> CArray
    requires std::is_same_v<T, char>
    {
        return CArray::from_string_view(std::string_view{c_str});
    }

    [[nodiscard]] auto as_cpp_view() const noexcept -> std::string_view
    requires std::is_same_v<T, char>
    {
        return {this->pointer, this->length};
    }

    operator std::string_view() const
    requires std::is_same_v<T, char>
    {
        return this->as_cpp_view();
    }
};

using CCharArray = CArray<char>;

static_assert(std::is_trivial_v<CCharArray>);
static_assert(std::is_standard_layout_v<CCharArray>);

static auto operator""_rust(char const* c_str, size_t len) -> CCharArray {
    return CCharArray::from_ptr_len(c_str, len);
}
}  // namespace log_mechanic

#endif  // LOGMECH_RUST_COMPAT_HPP
