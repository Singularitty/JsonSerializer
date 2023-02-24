# JsonSerializer
Two implementations of a Json serializer and deserializer in Rust and Go 

## Description

Each implementation contains the following features:
- Json Serializer
- Json Deserializer
- Json Acessor Aplication

## Usage

A variable of type Json must be defined according to the examples contained in the main of each implementation.

Acessors can also be defined which will be applied to the serialized json in order to obtain specific values or objects from said Json, these can also be defined according to the examples in main.

After defining a Json and an Acessor, two channels of type JC (Json Channel) need to be instanciated and three threads, serialize_json, eval and deserialize_json, need to be started. Each function in each thread needs to receive the respective channel endpoints according to the example in main.
