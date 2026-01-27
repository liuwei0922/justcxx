pub(crate) const CONTENT: &str = r#"
#pragma once
#include "rust/cxx.h"
#include <memory>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

namespace bridge_detail {
    // eigen support (only normal vector/matrix/array)
    template <typename T, typename = void>
    struct is_eigen_dense : std::false_type {};

    template <typename T>
    struct is_eigen_dense<T, std::void_t<
        decltype(std::declval<T>().data()),
        decltype(std::declval<T>().size()),
        typename T::Scalar
    >> : std::true_type {};

    // string
    template <typename T> struct is_string : std::false_type {};
    template <> struct is_string<std::string> : std::true_type {};

    // vector
    template <typename T> struct is_std_vector : std::false_type {};
    template <typename T> struct is_std_vector<std::vector<T>> : std::true_type {};

    // array
    template <typename T> struct is_std_array : std::false_type {};
    template <typename T, size_t N> struct is_std_array<std::array<T, N>> : std::true_type {};

    // unique_ptr
    template <typename T> struct is_unique_ptr : std::false_type {};
    template <typename T> struct is_unique_ptr<std::unique_ptr<T>> : std::true_type {};

    // map
    template <typename T> struct is_std_map : std::false_type {};
    template <typename K, typename V> struct is_std_map<std::unordered_map<K, V>> : std::true_type {};

    // optional
    template <typename T> struct is_optional : std::false_type {};
    template <typename T> struct is_optional<std::optional<T>> : std::true_type {};

    template <typename T>
    struct is_general_ref_type {
        using U = typename std::decay_t<T>;
        static constexpr bool value =
            !std::is_arithmetic_v<U> &&     
            !std::is_enum_v<U> &&           
            !is_string<U>::value &&         
            !is_std_vector<U>::value &&    
            !is_std_array<U>::value &&      
            !is_std_map<U>::value &&      
            !is_unique_ptr<U>::value &&   
            !is_optional<U>::value &&  
            !is_eigen_dense<U>::value;
    };

    // string must copy 
    inline rust::String return_convert(const std::string &s) {
        return rust::String(s);
    }

    inline rust::String return_convert(std::string &s) {
        return s;
    }

    // vector<Number> need slice
    template <typename T>
    inline
        typename std::enable_if_t<std::is_arithmetic_v<T>, rust::Slice<const T>>
        return_convert(const std::vector<T> &v) {
        return rust::Slice<const T>(v.data(), v.size());
    }

    // vector<Object> need reference
    template <typename T>
    inline typename std::enable_if_t<!std::is_arithmetic_v<T>, std::vector<T> &>
    return_convert(std::vector<T> &v) {
        return v;
    }

    // map need reference
    template <typename K, typename V>
    inline std::unordered_map<K, V>& return_convert(std::unordered_map<K, V>& m) {
        return m;
    }
    
    // map need reference
    template <typename K, typename V>
    inline const std::unordered_map<K, V>& return_convert(const std::unordered_map<K, V>& m) {
        return m;
    }

    // map owned must return unique_ptr
    template <typename K, typename V>
    inline std::unique_ptr<std::unordered_map<K, V>> return_convert(std::unordered_map<K, V>&& m) {
        return std::make_unique<std::unordered_map<K, V>>(std::move(m));
    }
    

    // array<Number,Size> is value, copy 
    template <typename T, size_t N>
    inline std::array<T, N> return_convert(const std::array<T, N> &v) {
        return v;
    }

    // scalar(Number|Enum) is value, copy
    template <typename T>
    inline
        typename std::enable_if_t<std::is_arithmetic_v<T> || std::is_enum_v<T>,
                                  T>
        return_convert(const T &val) {
        return val;
    }

    // Object must be referenced
    template <typename T>
    inline typename std::enable_if_t<is_general_ref_type<T>::value,
                                     T &>
    return_convert(T &val) {
        return val;
    }

    template <typename T>
    inline typename std::enable_if_t<is_general_ref_type<T>::value,
                                     const T &>
    return_convert(const T &val) {
        return val;
    }

    // Object owned must return unique_ptr
    template <typename T>
    inline typename std::enable_if_t<
        is_general_ref_type<typename std::decay_t<T>>::value, 
        std::unique_ptr<typename std::decay_t<T>>>
    return_convert(T &&val) {
        using ObjType = typename std::decay_t<T>;    
        return std::make_unique<ObjType>(std::forward<T>(val));
    }

    // smart pointer must return reference
    template <typename T>
    inline T& return_convert(std::unique_ptr<T>& ptr) {
        if (!ptr) throw std::runtime_error("Smart pointer is null");
        return *ptr;
    }

    template <typename T>
    inline const T& return_convert(const std::unique_ptr<T>& ptr) {
        if (!ptr) throw std::runtime_error("Smart pointer is null");
        return *ptr;
    }

    // smart pointer owned must return unique_ptr
    template <typename T>
    inline std::unique_ptr<T> return_convert(std::unique_ptr<T>&& ptr) {
        return std::move(ptr);
    }

    // optional<T>
    template <typename T>
    inline decltype(auto) return_convert(const std::optional<T>& opt) {
        if (!opt) throw std::runtime_error("Optional value is null");    
        return return_convert(*opt); 
    }

    
    template <typename T>
    inline decltype(auto) return_convert(std::optional<T>&& opt) {
        if (!opt) throw std::runtime_error("Optional value is null");
        return return_convert(std::move(*opt)); 
    }

    template <typename T>
    inline typename std::enable_if_t<
        is_eigen_dense<T>::value && 
        std::is_arithmetic_v<typename T::Scalar>, 
        rust::Slice<const typename T::Scalar>>
    return_convert(const T& v) {
        return rust::Slice<const typename T::Scalar>(v.data(), v.size());
    }
    
    // string must copy 
    inline std::string arg_convert(rust::Str s) { return std::string(s); }
    inline std::string arg_convert(rust::String s) { return std::string(s); }

    // &[T] need reconstruct vector<scalar>
    template <typename T>
    inline
        typename std::enable_if_t<std::is_arithmetic_v<T> || std::is_enum_v<T>,
                                  std::vector<T>>
        arg_convert(rust::Slice<const T> slice) {
        return std::vector<T>(slice.begin(), slice.end());
    }
 
    // &mut [T] need reconstruct vector<scalar>
    template <typename T>
    inline
        typename std::enable_if_t<std::is_arithmetic_v<T> || std::is_enum_v<T>,
                                  std::vector<T>>
        arg_convert(rust::Slice<T> slice) {
        return std::vector<T>(slice.begin(), slice.end());
    }
    // unique_ptr<T> 
    template <typename T>
    inline T arg_convert(std::unique_ptr<T> ptr) {
        if (!ptr) throw std::runtime_error("Argument is null");
        return std::move(*ptr);
    }

    // others using std::forward
    template <typename T>
    inline T &&arg_convert(T &&arg) {
        return std::forward<T>(arg);
    }

    template <typename L, typename R>
    inline typename std::enable_if_t<is_unique_ptr<L>::value>
    assign_smart(L& lhs, R&& rhs) {
        lhs = std::forward<R>(rhs);
    }

    template <typename L, typename R>
    inline typename std::enable_if_t<!is_unique_ptr<L>::value && is_unique_ptr<std::decay_t<R>>::value>
    assign_smart(L& lhs, R&& rhs) {
        lhs = std::move(*rhs);
    }
    
    template <typename L, typename R>
    inline typename std::enable_if_t<!is_unique_ptr<L>::value && !is_unique_ptr<std::decay_t<R>>::value>
    assign_smart(L& lhs, R&& rhs) {
        lhs = arg_convert(std::forward<R>(rhs));
    }
} // namespace bridge_detail

#define DEFINE_VAL(CLASS, FIELD)                                               \
    inline auto CLASS##_get_##FIELD(const CLASS &obj)                          \
        -> decltype(::bridge_detail::return_convert(obj.FIELD)) {              \
        return ::bridge_detail::return_convert(obj.FIELD);                     \
    }

#define DEFINE_OBJ(CLASS, FIELD)                                               \
    inline auto CLASS##_get_##FIELD(CLASS &obj)                                \
        -> decltype(::bridge_detail::return_convert(obj.FIELD)) {              \
        return ::bridge_detail::return_convert(obj.FIELD);                     \
    }

#define DEFINE_OBJ_CONST(CLASS, FIELD)                                         \
    inline auto CLASS##_get_##FIELD(const CLASS &obj)                          \
        -> decltype(::bridge_detail::return_convert(obj.FIELD)) {              \
        return ::bridge_detail::return_convert(obj.FIELD);                     \
    }

#define DEFINE_OBJ_SET(CLASS, FIELD)                                           \
    template <typename Arg>                                                    \
    inline void CLASS##_set_##FIELD(CLASS &obj, Arg val) {                     \
        ::bridge_detail::assign_smart(obj.FIELD, std::move(val));              \
    }

#define DEFINE_VAL_SET(CLASS, FIELD)                                           \
    template <typename T>                                                      \
    inline void CLASS##_set_##FIELD(CLASS &obj, T val) {                       \
        obj.FIELD = ::bridge_detail::arg_convert(val);                         \
    }

#define DEFINE_ITER(CLASS, FIELD, ITEM_TYPE)                                          \
    struct CLASS##_##FIELD##_IterCtx {                                               \
        using IterType = decltype(std::declval<CLASS &>().begin());            \
        IterType cur;                                                          \
        IterType end;                                                          \
        CLASS##_##FIELD##_IterCtx(CLASS &obj) : cur(obj.begin()), end(obj.end()) {}  \
    };                                                                         \
    inline std::unique_ptr<CLASS##_##FIELD##_IterCtx> CLASS##_##FIELD##_iter_new(         \
        CLASS &obj) {                                                          \
        return std::make_unique<CLASS##_##FIELD##_IterCtx>(obj);                     \
    }                                                                          \
    inline std::unique_ptr<ITEM_TYPE> CLASS##_##FIELD##_iter_next(                  \
        CLASS##_##FIELD##_IterCtx &ctx) {                                   \
        if (ctx.cur == ctx.end)                                                \
            return nullptr;                                                    \
        auto ptr = std::make_unique<ITEM_TYPE>(std::move(*ctx.cur));           \
        ++ctx.cur;                                                             \
        return ptr;                                                            \
    }

#define DEFINE_OPT_VAL(CLASS, FIELD)                                           \
    inline auto CLASS##_get_##FIELD(const CLASS &obj)                          \
        -> decltype(::bridge_detail::return_convert(*obj.FIELD)) {             \
        if (!obj.FIELD)                                                        \
            throw std::runtime_error(#FIELD " is nullopt");                    \
        return ::bridge_detail::return_convert(*obj.FIELD);                    \
    }

#define DEFINE_OPT_OBJ(CLASS, FIELD)                                           \
    inline auto CLASS##_get_##FIELD(CLASS &obj)                          \
        -> decltype(::bridge_detail::return_convert(*obj.FIELD)) {             \
        if (!obj.FIELD)                                                        \
            throw std::runtime_error(#FIELD " is nullopt");                    \
        return ::bridge_detail::return_convert(*obj.FIELD);                    \
    }

#define DEFINE_OPT_OBJ_CONST(CLASS, FIELD)                                         \
    inline auto CLASS##_get_##FIELD(const CLASS &obj)                          \
        -> decltype(::bridge_detail::return_convert(*obj.FIELD)) {             \
        if (!obj.FIELD)                                                        \
            throw std::runtime_error(#FIELD " is nullopt");                    \
        return ::bridge_detail::return_convert(*obj.FIELD);                    \
    }


#define DEFINE_METHOD(CLASS, RUST_NAME, CPP_METHOD) \
    template <typename... Args> \
    inline decltype(auto) CLASS##_method_##RUST_NAME(CLASS &obj, Args... args) { \
        if constexpr (std::is_void_v<decltype(obj.CPP_METHOD(::bridge_detail::arg_convert(std::forward<Args>(args))...))>) { \
            obj.CPP_METHOD(::bridge_detail::arg_convert(std::forward<Args>(args))...); \
        } else { \
            return ::bridge_detail::return_convert( \
                obj.CPP_METHOD(::bridge_detail::arg_convert(std::forward<Args>(args))...) \
            ); \
        } \
    }

#define DEFINE_METHOD_CONST(CLASS, RUST_NAME, CPP_METHOD) \
    template <typename... Args> \
    inline decltype(auto) CLASS##_method_##RUST_NAME(const CLASS &obj, Args... args) { \
        if constexpr (std::is_void_v<decltype(obj.CPP_METHOD(::bridge_detail::arg_convert(std::forward<Args>(args))...))>) { \
            obj.CPP_METHOD(::bridge_detail::arg_convert(std::forward<Args>(args))...); \
        } else { \
            return ::bridge_detail::return_convert( \
                obj.CPP_METHOD(::bridge_detail::arg_convert(std::forward<Args>(args))...) \
            ); \
        } \
    }

#define DEFINE_OP_CALL(CLASS, RUST_NAME)                                       \
    template <typename... Args>                                                \
    inline auto CLASS##_method_##RUST_NAME(CLASS &obj, Args... args)           \
        -> decltype(obj(args...)) {                                            \
        return obj(args...);                                                   \
    }

#define DEFINE_OP_CALL_CONST(CLASS, RUST_NAME)                                 \
    template <typename... Args>                                                \
    inline auto CLASS##_method_##RUST_NAME(const CLASS &obj, Args... args)     \
        -> decltype(obj(args...)) {                                            \
        return obj(args...);                                                   \
    }

#define DEFINE_STATIC_METHOD(CLASS, RUST_NAME, CPP_METHOD)                     \
    template <typename... Args>                                                \
    inline auto CLASS##_method_##RUST_NAME(Args... args)                       \
        -> decltype(CLASS::CPP_METHOD(args...)) {                              \
        return CLASS::CPP_METHOD(args...);                                     \
    }

#define DEFINE_CTOR(CLASS, FUNC_NAME)                                          \
    template <typename... Args>                                                \
    inline std::unique_ptr<CLASS> make_##CLASS##_##FUNC_NAME(Args... args) {   \
        return std::make_unique<CLASS>(::bridge_detail::arg_convert(args)...); \
    }

#define DEFINE_VEC_LEN(VEC_TYPE) \
    inline size_t VEC_TYPE##_len(const VEC_TYPE& self) { return self.size(); }

#define DEFINE_VEC_GET(VEC_TYPE) \
    inline decltype(auto) VEC_TYPE##_get(VEC_TYPE& self, size_t i) { \
        return ::bridge_detail::return_convert(self[i]); \
    }

#define DEFINE_VEC_PUSH(VEC_TYPE, ELEM_TYPE) \
    template <typename Arg> \
    inline void VEC_TYPE##_push(VEC_TYPE& self, Arg&& val) { \
        self.push_back(::bridge_detail::arg_convert(std::forward<Arg>(val))); \
    }

#define DEFINE_VEC_OPS(VEC_TYPE, ELEM_TYPE) \
    DEFINE_VEC_LEN(VEC_TYPE) \
    DEFINE_VEC_GET(VEC_TYPE) \
    DEFINE_VEC_PUSH(VEC_TYPE, ELEM_TYPE)

#define DEFINE_MAP_ITER(MAP_TYPE) \
    struct MAP_TYPE##_IterCtx { \
        using IterType = typename MAP_TYPE::iterator; \
        IterType cur; \
        IterType end; \
        MAP_TYPE##_IterCtx(MAP_TYPE& m) : cur(m.begin()), end(m.end()) {} \
    }; \
    inline auto MAP_TYPE##_iter_new(MAP_TYPE& m) { \
        return std::make_unique<MAP_TYPE##_IterCtx>(m); \
    } \
    inline decltype(auto) MAP_TYPE##_iter_key(MAP_TYPE##_IterCtx& ctx) { \
        return ::bridge_detail::return_convert(ctx.cur->first); \
    } \
    inline decltype(auto) MAP_TYPE##_iter_val(MAP_TYPE##_IterCtx& ctx) { \
        return ::bridge_detail::return_convert(ctx.cur->second); \
    } \
    inline void MAP_TYPE##_iter_step(MAP_TYPE##_IterCtx& ctx) { \
        ++ctx.cur; \
    } \
    inline bool MAP_TYPE##_iter_is_end(MAP_TYPE##_IterCtx& ctx) { \
        return ctx.cur == ctx.end; \
    }

#define DEFINE_MAP_LEN(MAP_TYPE) \
    inline size_t MAP_TYPE##_len(const MAP_TYPE& self) { return self.size(); }

#define DEFINE_MAP_GET(MAP_TYPE) \
    template <typename ArgKey> \
    inline decltype(auto) MAP_TYPE##_get(MAP_TYPE& self, ArgKey key) { \
        auto cpp_key = ::bridge_detail::arg_convert(key); \
        auto it = self.find(cpp_key); \
        if (it == self.end()) throw std::out_of_range("Key not found"); \
        return ::bridge_detail::return_convert(it->second); \
    }

#define DEFINE_MAP_INSERT(MAP_TYPE, KEY_TYPE, VAL_TYPE) \
    template <typename KeyArg, typename ValArg> \
    inline void MAP_TYPE##_insert(MAP_TYPE& self, KeyArg&& key, ValArg&& val) { \
        self.insert_or_assign( \
            ::bridge_detail::arg_convert(std::forward<KeyArg>(key)), \
            ::bridge_detail::arg_convert(std::forward<ValArg>(val)) \
        ); \
    }

#define DEFINE_MAP_OPS(MAP_TYPE) \
    DEFINE_MAP_LEN(MAP_TYPE) \
    DEFINE_MAP_GET(MAP_TYPE) \


"#;
