#pragma once
#include <memory>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>
#include "rust/cxx.h"

struct Config {
    int id;
    float value;
    std::string name;
    Config(){
        id = 42;
        value = 56.0;
        name = "test";
    }
};


struct Manager{
    Config config;
    Manager(){
        config = Config();
    }
};

struct School {
    Manager teacher;
    Manager student;
    std::unique_ptr<Manager> other;

    School() {
        teacher = Manager();
        student = Manager();
        other = std::make_unique<Manager>();
    }
};

struct Methods{
    int id;
    Config config;
    int get_id() const {
        return id;
    }
    void set_id(int v) {
        id = v;
    }
    const Config& get_config() const {
        return config;
    }
    void set_config(const Config& c) {
        config = c;
    }
    void set_config_by_value(Config c) {
        config = c;
    }
    static int add(int v, int w) {
        return v + w;
    }
    Config create_config() const {
        return config;
    }
    std::optional<int> optional_id(bool flag) const {
        return flag ? std::optional<int>(id) : std::nullopt;
    }
    
    Methods() = default;
};

struct IntContainer {
    std::vector<int> data = {10, 20, 30};
    auto begin() { return data.begin(); }
    auto end() { return data.end(); }
    auto begin() const { return data.begin(); }
    auto end() const { return data.end(); }
};

struct ConfigContainer {
    std::vector<Config> data;
    std::vector<std::string> names;
    ConfigContainer() {
        data.emplace_back();
        data.back().id = 100;
        names.emplace_back("100");
        data.emplace_back();
        data.back().id = 200;
        names.emplace_back("200");
    }
    auto begin() { return data.begin(); }
    auto end() { return data.end(); }
    auto begin() const { return data.begin(); }
    auto end() const { return data.end(); }
};


struct Chance{
    std::optional<int> probability;
    Chance(){
        probability = 75;
    }
    void set_chance(bool has_chance, int value){
        if(has_chance){
            probability = value;
        }else{
            probability = std::nullopt;
        }
    }
};

struct Wallet{
    std::optional<Config> config;
    Wallet(){
        config = std::nullopt;
    }
    void set_config(bool has_config, const Config& c){
        if(has_config){
            config = c;
        }else{
            config = std::nullopt;
        }
    }
};


struct MapExample{
    std::unordered_map<int, std::string> int_str_map;
    std::unordered_map<int, Config> int_config_map;
    MapExample(){
        int_config_map[10] = Config();
        int_config_map[10].id = 10;
        int_config_map[20] = Config();
        int_config_map[20].id = 20;

        int_str_map[1] = "one";
        int_str_map[2] = "two";
    }
};