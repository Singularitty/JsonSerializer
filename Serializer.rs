/**
 * Fundamentos de Linguagens de Programacao
 * Asssignment 6 - Message-passing Concurrent Programming in Go and Rust
 * Section 1 - Rust
 * Luis Ferreirinha N51127
 */


use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;


#[derive(Debug)]
enum Json {
    Number(f64),
    String(String),
    Boolean(bool),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>), // used instead of HashMap in an attempt to preserve order in the json object
    Null,
}

enum JC {
    Number(f64),
    String(String),
    Boolean(bool),
    ArrayLen(usize), // Informs about the size of the array or object that is about to be received
    Null,
    ArrayStart,
    ArrayEnd,
    ObjectStart,
    ObjectEnd,
    Stream(mpsc::Receiver<JC>), // Used to receive the values of Arrays or Objects
}

/**
 * Protocols:
 * Simple values: Sent in their own JC
 * Array: ArrayStart -> ArrayLen -> (Stream channel for each array value) -> ArrayEnd
 * Object: ObjectStart -> ArrayLen -> (for each object: String -> Stream channel) -> ArrayEnd
 * Note: Map acessor does not strictly follow the array protocol, but this is accounted for in the deserializer function
 */
fn serialise_json(val: &Json, sender: mpsc::Sender<JC>) {

    match val {
        Json::Number(num) => {
            sender.send(JC::Number(*num)).unwrap();
        },
        Json::String(str) => {
            sender.send(JC::String(str.to_string())).unwrap();
        },
        Json::Boolean(bol) => {
            sender.send(JC::Boolean(*bol)).unwrap();
        },
        Json::Null => {
            sender.send(JC::Null).unwrap();
        },
        Json::Array(json_array) => {
            sender.send(JC::ArrayStart).unwrap();
            sender.send(JC::ArrayLen(json_array.len())).unwrap();
            for json_value in json_array {
                // Serialise the contents recursively by creating a new channel and 
                // sending a receiving channel endpoint, whilist sending the contents
                // of the value throught the new channel
                let (array_sender, array_receiver) = mpsc::channel::<JC>();
                sender.send(JC::Stream(array_receiver)).unwrap();
                serialise_json(json_value, array_sender);
            }
            sender.send(JC::ArrayEnd).unwrap();
        },
        Json::Object(json_object) => {
            sender.send(JC::ObjectStart).unwrap();
            sender.send(JC::ArrayLen(json_object.len())).unwrap();
            for (key, json_value) in json_object {
                sender.send(JC::String(key.to_string())).unwrap();
                // Serialise the contents recursively by creating a new channel and 
                // sending a receiving channel endpoint, whilist sending the contents
                // of the value throught the new channel
                let (object_sender, object_receiver) = mpsc::channel::<JC>();
                sender.send(JC::Stream(object_receiver)).unwrap();
                serialise_json(json_value, object_sender);
            }
            sender.send(JC::ObjectEnd).unwrap();
        }
    }
}


fn deserialise_json(receiver: mpsc::Receiver<JC>) -> Json {
    match receiver.recv().unwrap() {
        JC::Null => return Json::Null,
        JC::Number(num) => return Json::Number(num),
        JC::String(str) => return Json::String(str.to_string()),
        JC::Boolean(b) => return Json::Boolean(b),
        JC::ArrayStart => {
            let mut array: Vec<Json> = vec![];
            match receiver.recv().unwrap() {
                JC::ArrayLen(arrlen) => {
                    for _ in 0..arrlen {
                        match receiver.recv().unwrap() {
                            // If we receive a stream we know its an object or Array so we will
                            // deserialise recursively
                            JC::Stream(value_receiver) => {
                                let new_json_value = deserialise_json(value_receiver);
                                array.push(new_json_value);
                            },
                            // Because Map Acessor does not send streams for values that are not arrays or objects
                            // We need to expect the simpler values in deserialise
                            JC::Number(num) => array.push(Json::Number(num)),
                            JC::Null => array.push(Json::Null),
                            JC::String(str) => array.push(Json::String(str)),
                            JC::Boolean(bol) => array.push(Json::Boolean(bol)),
                            _ => panic!("Expected a JC Stream or a simple value (Deserialise Array)"),
                        }
                    }
                    match receiver.recv().unwrap() {
                        JC::ArrayEnd => {
                            return Json::Array(array);
                        },
                        _ => panic!("Expected ArrayEnd (Deserialise Array)")
                        }
                },
                _ => panic!("Expected ArryLen (Deserialise Array)"),
            }
        },
        JC::ObjectStart => {
            let mut json_object: BTreeMap<String, Json> = BTreeMap::new();
            match receiver.recv().unwrap() {
                JC::ArrayLen(maplen) => {
                    for _ in 0..maplen {
                        match  receiver.recv().unwrap() {
                            JC::String(key) => {
                                match receiver.recv().unwrap() {
                                    // If we receive a stream we know its an object or Array so we will
                                    // deserialise recursively
                                    JC::Stream(value_receiver) => {
                                        let new_json_value = deserialise_json(value_receiver);
                                        json_object.insert(key, new_json_value);
                                    },
                                    // If we get a regular values just add it to the map
                                    // This is expected for arrays that result of Map acessor
                                    // but not expected for objects, but I will leave it here
                                    // just in case, since it doesnt hurt the implementation
                                    JC::Number(num) => {
                                        json_object.insert(key, Json::Number(num));
                                    },
                                    JC::Null => {
                                        json_object.insert(key, Json::Null);
                                    },
                                    JC::String(str) => {
                                        json_object.insert(key, Json::String(str));
                                    },
                                    JC::Boolean(bol) => {
                                        json_object.insert(key, Json::Boolean(bol));
                                    },
                                    _ => panic!("Expected a JC Stream or a simple value (Deserialise Object)"),
                                }
                            },
                            _ => panic!("Expected a key for a object field (Deserialise object)")
                            
                        }
                    }
                    match receiver.recv().unwrap() {
                        JC::ObjectEnd => {
                            return Json::Object(json_object);
                        },
                        _ => panic!("Expected a ObjectEnd (Deserialise Object)")
                    }
                },
                _ => panic!("Expected ArrayLen after ObjectStart (Deserialise Object)")
            }
        },
        _ => panic!("Unexpected ArrayEnd, ObjectEnd, ArryLen or Stream here. (deserialiser)")
    }
}

#[derive(Debug)]
enum Acessor {
    ObjectField(String, Box<Acessor>),
    ArrayEntry(usize, Box<Acessor>),
    Map(Box<Acessor>),
    End,
}


/**
 * This function is used to completly consume a value that is streamed thought the receiver channel
 * Used when applying acessor to arrays and objects, and we need to get rid of the values that do not
 * match the index or label in the acessor. Since these are streamed thought a receiver channel by the
 * serializer and these channels are rendvouz, the serialiser thread will hang waiting to send the rest of the
 * json value throught the channel, so this prevents it.
 */
fn consume_value(receiver: mpsc::Receiver<JC>) {
    match receiver.recv().unwrap() {
        JC::Number(_) => (), // Throw away the values
        JC::String(_) => (),
        JC::Boolean(_) => (),
        JC::Null => (),
        JC::ArrayStart => {
            match receiver.recv().unwrap() {
                JC::ArrayLen(arrlen) => {
                    for _ in 0..arrlen {
                        match receiver.recv().unwrap() {
                            JC::Stream(val_stream) => {
                                consume_value(val_stream);
                            },
                            _ => panic!("Something went wrong. Serialised Json packets are out of order")
                        }
                    }
                },
                _ => panic!("Something went wrong. Serialised Json packets are out of order")
            }
        },
        JC::ObjectStart => {
            match receiver.recv().unwrap() {
                JC::ArrayLen(maplen) => {
                    for _ in 0..maplen {
                        match receiver.recv().unwrap() {
                            JC::String(_) => {
                                match receiver.recv().unwrap() {
                                    JC::Stream(val_stream) => {
                                        consume_value(val_stream);
                                    },
                                    _ => panic!("Something went wrong. Serialised Json packets are out of order")
                                }
                            },
                            _ => panic!("Something went wrong. Serialised Json packets are out of order")
                        }
                    }
                },
                _ => panic!("Something went wrong. Serialised Json packets are out of order"),
            }
        },
        _ => panic!("Something went wrong. Serialised Json packets are out of order"),
    }
}


fn eval(acessor: &Acessor, receiver: mpsc::Receiver<JC>, sender: mpsc::Sender<JC>) {
    // Recursively apply acessor with each call of this function
    match acessor {
        Acessor::ObjectField(label , next_acessor) => {
            match receiver.recv().unwrap() {
                JC::ObjectStart => {
                    match receiver.recv().unwrap() {
                        JC::ArrayLen(maplen) => {
                            for _ in 0..maplen {
                                match receiver.recv().unwrap() {
                                    JC::String(obj_label) => {
                                        match receiver.recv().unwrap() {
                                            JC::Stream(value_stream) => {
                                                if obj_label.eq(label) {
                                                    // recursively handle the values obtained by applying an acesor
                                                    let sender_clone = sender.clone();
                                                    eval(next_acessor, value_stream, sender_clone);
                                                } else {
                                                    consume_value(value_stream); // Consume the stream and therefore the json value
                                                }
                                            },
                                            _ => panic!("Expected Object Stream (Eval ObjectField)")
                                        }
                                    },
                                    _ => panic!("Expected Object Label (Eval ObjectField)")
                                }
                            }
                            match receiver.recv().unwrap() {
                                JC::ObjectEnd => (), // consume object end
                                _ => panic!("Expected Object End (eval objectfield)")
                            }
                        },
                        _ => panic!("Expected ArrayLen (Eval ObjectField)")
                    }
                },
                _ => panic!("Cannot apply ObjectField Acessor to this Json Value")
            }
        },
        Acessor::ArrayEntry(index, next_acessor) => {
            match receiver.recv().unwrap() {
                JC::ArrayStart => {
                    match receiver.recv().unwrap() {
                        JC::ArrayLen(arrlen) => {
                            for i in 0..arrlen {
                                match receiver.recv().unwrap() {
                                    JC::Stream(value_stream) => {
                                        if i == *index {
                                            // recursively handle the values obtained by applying an acesor
                                            let sender_clone = sender.clone();
                                            eval(next_acessor, value_stream, sender_clone);
                                        } else {
                                            consume_value(value_stream); // consume the stream and the value
                                        }
                                    },
                                    _ => panic!("Expected Array Value Stream (Eval ArrayEntry)")
                                }
                            }
                            match receiver.recv().unwrap() {
                                JC::ArrayEnd => (), // Consume Array End
                                _ => panic!("Expected Array End (Eval ArrayEntry)")
                            }
                        },
                        _ => panic!("Expected ArrayLen (Eval ArrayEntry)")
                    }
                },
                _ => panic!("Cannot apply Index accessor to this Json Value")
            }
        },
        Acessor::Map(next_acessor) => {
            // Must send a new array composed of the results of the acessor
            match receiver.recv().unwrap() {
                JC::ArrayStart => {
                    sender.send(JC::ArrayStart).unwrap(); // Signal the start of a new array
                    match receiver.recv().unwrap() {
                        JC::ArrayLen(arrlen) => {
                            sender.send(JC::ArrayLen(arrlen)).unwrap(); // send length of array
                            for _ in 0..arrlen {
                                match receiver.recv().unwrap() {
                                    JC::Stream(value_stream) => {
                                        // recursively handle the values obtained by applying the next acessor to each
                                        // value of the array
                                        let sender_clone = sender.clone();
                                        eval(next_acessor, value_stream, sender_clone);
                                    },
                                    _ => panic!("Expected Value Stream (Eval Map)")
                                }
                            }
                            match receiver.recv().unwrap() {
                                JC::ArrayEnd => sender.send(JC::ArrayEnd).unwrap(), // send array end
                                _ => panic!("Expected array end (Eval Map)")
                            }
                        },
                        _ => panic!("Expected ArrayLen (Eval Map)")
                    }
                },
                _ => panic!("Cannot apply Map to this Json Value")
            }
        },
        Acessor::End => {
            // When End acessor is reached, we simply pass along the result of applying the previous acessors
            // to the sender channel
            let serial_json_packet = receiver.recv().unwrap();
            match serial_json_packet {
                JC::Number(_) => sender.send(serial_json_packet).unwrap(),
                JC::String(_) => sender.send(serial_json_packet).unwrap(),
                JC::Boolean(_) => sender.send(serial_json_packet).unwrap(),
                JC::Null => sender.send(serial_json_packet).unwrap(),
                JC::ArrayStart => {
                    sender.send(serial_json_packet).unwrap(); // Send Array Start
                    match receiver.recv().unwrap() {
                        JC::ArrayLen(arrlen) => {
                            sender.send(JC::ArrayLen(arrlen)).unwrap(); // Send Array Len
                            for _ in 0..arrlen {
                                match receiver.recv().unwrap() {
                                    JC::Stream(value_stream) => sender.send(JC::Stream(value_stream)).unwrap(),
                                    _ => panic!("Expected Array Value Stream (eval end array)")
                                }
                            }
                            match receiver.recv().unwrap() {
                                JC::ArrayEnd => sender.send(JC::ArrayEnd).unwrap(), // Send Array End
                                _ => panic!("Expected Array End (Eval End Array)")
                            }
                        },
                        _ => panic!("Expected a ArrayLen (Eval End Array)")
                    }
                },
                JC::ObjectStart => {
                    sender.send(serial_json_packet).unwrap(); // Send Object Start
                    match receiver.recv().unwrap() {
                        JC::ArrayLen(maplen) => {
                            sender.send(JC::ArrayLen(maplen)).unwrap(); // Send Array Len
                            for _ in 0..maplen {
                                let key = receiver.recv().unwrap();
                                match key {
                                    JC::String(_) => sender.send(key).unwrap(), // Send key
                                    _ => panic!("Expected key for object value (Eval End of Object)")
                                }
                                match receiver.recv().unwrap() {
                                    JC::Stream(value_stream) => sender.send(JC::Stream(value_stream)).unwrap(),
                                    _ => panic!("Expected Object Value Stream (Eval End of Object)")
                                }
                            }
                            match receiver.recv().unwrap() {
                                JC::ObjectEnd => sender.send(JC::ObjectEnd).unwrap(), // Send Object End
                                _ => panic!("Expected Object End (Eval End of Object)")
                            }
                        },
                        _ => panic!("Expected ArrayLen (Eval End of Object)")
                    }
                },
                _ => panic!("Unexpected ArrayEnd, ObjectEnd, ArryLen or Stream here. (eval End)")

            }
        }
    }
}

fn Printer(json: &Json, depth: usize) {
    let mut indent = "".to_string();
    for i in 0..depth {
        indent = indent + " ";
    }
    match json {
        Json::Null => println!("{}", indent),
        Json::String(s) => println!("{}\"{}\"", indent, s),
        Json::Number(n) => println!("{}{}", indent, n),
        Json::Boolean(b) => println!("{}{}", indent, b),
        Json::Array(arr) => {
            println!("{}[", indent);
            for jval in arr {
                Printer(jval, depth+1);
            }
            println!("{}]", indent);
        },
        Json::Object(map) => {
            println!("{}{{", indent);
            for (key, jval) in map.iter() {
                print!(" {}\"{}\": ", indent, key);
                Printer(jval, depth+1);
            }
            println!("{}}}", indent);
        }
    }
}

fn main() {
    

    let socialProfiles = Json::Array(vec![
            Json::Object(BTreeMap::from([
                ("name".to_string(), Json::String("Twitter".to_string())),
                ("link".to_string(), Json::String("https://twitter.com".to_string()))
            ])),
            Json::Object(BTreeMap::from([
                ("name".to_string(), Json::String("Facebook".to_string())),
                ("link".to_string(), Json::String("https://www.facebook.com".to_string()))
            ]))
        ]);

    let address = Json::Object(BTreeMap::from([
        ("city".to_string(), Json::String("New York".to_string())),
        ("postalCode".to_string(), Json::Number(64780.0)),
        ("Country".to_string(), Json::String("USA".to_string())),
    ]));

    let languages = Json::Array(vec![
        Json::String("Java".to_string()),
        Json::String("Node.js".to_string()),
        Json::String("Javascript".to_string()),
        Json::String("JSON".to_string()),
    ]);

    let json_test = Json::Object(BTreeMap::from([
        ("name".to_string(), Json::String("Jason Ray".to_string())),
        ("profession".to_string(), Json::String("Software Enginner".to_string())),
        ("age".to_string(), Json::Number(31.0)),
        ("address".to_string(), address),
        ("languages".to_string(), languages),
        ("socialProfiles".to_string(), socialProfiles),
    ]));


    // Prints Json object
    println!("Original Json:");
    Printer(&json_test, 0);


    // Acessor to apply to the json object
    // Uncomment the one you which to apply and leave the others commented

    // ."socialProfiles" End
    let acessor = Acessor::ObjectField("socialProfiles".to_string(), Box::new(Acessor::End));

    // ."socialProfiles" [0] End
    //let acessor = Acessor::ObjectField("socialProfiles".to_string(), Box::new(Acessor::ArrayEntry(0, Box::new(Acessor::End))));

    // ."socialProfiles" Map ."name" End
    //let acessor = Acessor::ObjectField("socialProfiles".to_string(), Box::new(Acessor::Map(Box::new(Acessor::ObjectField("name".to_string(), Box::new(Acessor::End))))));

    // ."address" ."postalCode" End
    //let acessor = Acessor::ObjectField("address".to_string(), Box::new(Acessor::ObjectField("postalCode".to_string(), Box::new(Acessor::End))));

    // ."age" End
    //let acessor = Acessor::ObjectField("age".to_string(), Box::new(Acessor::End));

    // ."languages" [3] End
    //let acessor = Acessor::ObjectField("languages".to_string(), Box::new(Acessor::ArrayEntry(3, Box::new(Acessor::End))));


    // Prints the acessor
    println!("\nJson after applying acessor: ");
    println!("{:?}\n", acessor);


    let (sender_1, receiver_1) = mpsc::channel::<JC>();
    let (sender_2, receiver_2) = mpsc::channel::<JC>();

    let handle_serialiser = thread::spawn(move || {
        serialise_json(&json_test, sender_1);
    });

    let handle_eval = thread::spawn(move || {
        eval(&acessor, receiver_1, sender_2);
    });

    let handle_deserialiser = thread::spawn(move || {
        let final_json = deserialise_json(receiver_2);
        // Prints the final json value
        Printer(&final_json, 0);
    });

    // wait for threads to finish
    handle_serialiser.join().unwrap();
    handle_eval.join().unwrap();
    handle_deserialiser.join().unwrap();

}
