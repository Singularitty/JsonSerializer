/**
 * Luis Ferreirinha
 */

package main

import "fmt"

// Since the type system isnt as robust as rusts, we need to have a way to
// identify which type of value a struct is supposed to carry, since we do
// not want to have multiple json values associated to a single json value variable

type JsonValueType int

// Identifies type of json value
const (
	Number JsonValueType = iota
	String
	Boolean
	Array
	Object
	Null
)

type Json struct {
	ValueType JsonValueType // Used to identify type of json value
	Number    float64
	String    string
	Boolean   bool
	Array     []Json
	Object    map[string]Json
	Null      *struct{} // Uses no memory
}

// Control Symbols

type Control int

// Same as above, need to identify which field of the struct contains data
// Aditionally used to model complex values with a simple protocol
const (
	NumVal Control = iota
	StrVal
	BoolVal
	NullVal
	ArrayStart
	ArrayLen
	ArrayEnd
	ObjectStart
	ObjectEnd
)

type JC struct {
	Number        float64
	String        string
	Boolean       bool
	ControlSymbol Control
	ArrayLen      int
}

/**
 * Protocols:
 * Simple values: Sent in their own JC
 * Array: ArrayStart -> ArrayLen -> (each value is sent) -> ArrayEnd
 * Object: ObjectStart -> ArrayLen -> (for each object: String -> value) -> ArrayEnd
 */
func serialise_json(val Json, sender chan<- JC) {
	switch val.ValueType {
	case Number:
		sender <- JC{Number: val.Number, ControlSymbol: NumVal}
	case String:
		sender <- JC{String: val.String, ControlSymbol: StrVal}
	case Boolean:
		sender <- JC{Boolean: val.Boolean, ControlSymbol: BoolVal}
	case Null:
		sender <- JC{ControlSymbol: NullVal}
	case Array:
		sender <- JC{ControlSymbol: ArrayStart}
		sender <- JC{ControlSymbol: ArrayLen, ArrayLen: len(val.Array)}
		for _, jval := range val.Array {
			// recursively handle each value
			serialise_json(jval, sender)
		}
		sender <- JC{ControlSymbol: ArrayEnd}
	case Object:
		sender <- JC{ControlSymbol: ObjectStart}
		sender <- JC{ControlSymbol: ArrayLen, ArrayLen: len(val.Object)}
		for key, jval := range val.Object {
			sender <- JC{ControlSymbol: StrVal, String: key}
			// recursively handle each value
			serialise_json(jval, sender)
		}
		sender <- JC{ControlSymbol: ObjectEnd}
	}
}

func deserialise_json(receiver <-chan JC) Json {
	serial_json_packet := <-receiver
	var json_value Json // Initialize var for Json Value
	switch serial_json_packet.ControlSymbol {
	case NumVal:
		json_value = Json{ValueType: Number, Number: serial_json_packet.Number}
	case StrVal:
		json_value = Json{ValueType: String, String: serial_json_packet.String}
	case BoolVal:
		json_value = Json{ValueType: Boolean, Boolean: serial_json_packet.Boolean}
	case NullVal:
		json_value = Json{ValueType: Null, Null: nil}
	case ArrayStart:
		var json_array []Json
		arraylen := (<-receiver).ArrayLen
		for i := 0; i < arraylen; i++ {
			// recursively handle each value
			json_array = append(json_array, deserialise_json(receiver))
		}
		json_value = Json{ValueType: Array, Array: json_array}
		if (<-receiver).ControlSymbol != ArrayEnd {
			panic("Expected a ArrayEnd (Array deserialiser)")
		}
	case ObjectStart:
		json_object := make(map[string]Json)
		maplen := (<-receiver).ArrayLen
		for i := 0; i < maplen; i++ {
			serialised_key := <-receiver
			// Panic if we dont get a key
			if serialised_key.ControlSymbol != StrVal {
				panic("Expected a string (Object deserialiser)")
			}
			// recursively handle each value
			json_object[serialised_key.String] = deserialise_json(receiver)
		}
		json_value = Json{ValueType: Object, Object: json_object}
		if (<-receiver).ControlSymbol != ObjectEnd {
			panic("Expected a ObjectEnd (Object deserialiser)")
		}
	default:
		panic("Unexpected ArrayEnd or ObjectEnd (deserialiser)")
	}
	return json_value
}

type AccessorType int

const (
	ObjectField AccessorType = iota
	ArrayEntry
	Map
	End
)

// In order to have a recursive type the struct must have a pointer to the next Accessor

type Accessor struct {
	Type         AccessorType
	Label        string
	Index        int
	NextAccessor *Accessor
}

func consumeValue(receiver <-chan JC) {
	serial_json_packet := <-receiver // consume the first value in order to match controlsymbol
	switch serial_json_packet.ControlSymbol {
	case NullVal: // Do nothing, value was already consumed
	case StrVal: // Do nothing
	case NumVal: // Do nothing
	case BoolVal: // Do nothing
	case ArrayStart:
		arrlen := (<-receiver).ArrayLen // consume array len
		for i := 0; i < arrlen; i++ {
			consumeValue(receiver) // consume each json value inside the array
		}
		<-receiver // consume array end
	case ObjectStart:
		maplen := (<-receiver).ArrayLen // consume array len
		for i := 0; i < maplen; i++ {
			<-receiver             // consume key
			consumeValue(receiver) // consume each json value in the object
		}
		<-receiver // consume object end
	default: // if we get objectend, arrayend or arraylen there is a problem
		panic("Something went wrong. Serialised Json packets are out of order")
	}

}

func eval(accessor Accessor, receiver <-chan JC, sender chan<- JC) {
	serial_json_packet := <-receiver
	switch accessor.Type {
	case ObjectField: // .s Accessor
		if serial_json_packet.ControlSymbol == ObjectStart {
			maplen := (<-receiver).ArrayLen
			for i := 0; i < maplen; i++ {
				serialised_key := <-receiver
				if serialised_key.ControlSymbol != StrVal {
					panic("Expected a string (Eval of Accessor ObjectField of Object Value)")
				}
				if serialised_key.String == accessor.Label {
					nextAccessor := accessor.NextAccessor
					eval(*nextAccessor, receiver, sender)
				} else {
					consumeValue(receiver)
				}
			}
			<-receiver // consume object end
		} else {
			panic("Can only apply ObjectField Accessors to Object Values")
		}
	case ArrayEntry: // [n] Accessor
		if serial_json_packet.ControlSymbol == ArrayStart {
			arraylen := (<-receiver).ArrayLen
			for i := 0; i < arraylen; i++ {
				if accessor.Index == i {
					nextAcessor := accessor.NextAccessor
					eval(*nextAcessor, receiver, sender)
				} else {
					consumeValue(receiver)
				}

			}
			<-receiver // consume array end
		} else {
			panic("Can only apply ArrayIndex Accessor to Array values")
		}
	case Map: // Map Accessor
		if serial_json_packet.ControlSymbol == ArrayStart {
			sender <- serial_json_packet
			serial_arrlen := <-receiver
			sender <- serial_arrlen
			arrlen := serial_arrlen.ArrayLen
			nextAcessor := accessor.NextAccessor
			for i := 0; i < arrlen; i++ {
				eval(*nextAcessor, receiver, sender)
			}
			arrend := <-receiver // consume array end
			if arrend.ControlSymbol != ArrayEnd {
				panic("Expected array end (Eval of Accessor Map of Array Value)")
			} else {
				sender <- arrend
			}
		} else {
			panic("Can only apply Map Accesor to Array Values")
		}
	case End: // End Accessor, sends the resulting json value to sender
		switch serial_json_packet.ControlSymbol {
		case NumVal:
			sender <- serial_json_packet
		case BoolVal:
			sender <- serial_json_packet
		case NullVal:
			sender <- serial_json_packet
		case StrVal:
			sender <- serial_json_packet
		case ArrayStart:
			sender <- serial_json_packet // send ArrayStart
			array_length_packet := <-receiver
			sender <- array_length_packet // send ArrayLen
			arrlen := array_length_packet.ArrayLen
			for i := 0; i < arrlen; i++ {
				eval(accessor, receiver, sender)
			}
			close_array_packet := <-receiver
			if close_array_packet.ControlSymbol == ArrayEnd {
				sender <- close_array_packet // send ArrayEnd
			} else {
				panic("Expected ArrayEnd (Eval of Accessor End of Value Array)")
			}
		case ObjectStart:
			sender <- serial_json_packet // send ObjectStart
			map_length_packet := <-receiver
			sender <- map_length_packet
			maplen := map_length_packet.ArrayLen
			for i := 0; i < maplen; i++ {
				serial_key := <-receiver
				if serial_key.ControlSymbol != StrVal {
					panic("Expected a string (Eval of Accesor End of Value object)")
				}
				sender <- serial_key
				eval(accessor, receiver, sender)
			}
			close_object_packet := <-receiver
			if close_object_packet.ControlSymbol == ObjectEnd {
				sender <- close_object_packet
			} else {
				panic("Expected ObjectEnd (Eval of Accessor End of Value object)")
			}
		default:
			panic("Unexpected ObjectEnd, ArrayEnd or ArrayLen ControlSymbol in Eval of Accessor End")
		}

	}
}

func Printer(value Json, depth int) {
	indent := ""
	for i := 0; i < depth; i++ {
		indent = indent + " "
	}
	switch value.ValueType {
	case String:
		fmt.Printf("%s\"%s\"\n", indent, value.String)
	case Null:
		fmt.Printf("%s\n", indent)
	case Boolean:
		fmt.Printf("%s%v\n", indent, value.Boolean)
	case Number:
		fmt.Printf("%s%v\n", indent, value.Number)
	case Array:
		fmt.Printf("%s[\n", indent)
		for _, jval := range value.Array {
			Printer(jval, depth+1)
		}
		fmt.Printf("%s]\n", indent)
	case Object:
		fmt.Printf("%s{\n", indent)
		for key, jval := range value.Object {
			fmt.Printf(" %s\"%s\": ", indent, key)
			Printer(jval, depth+1)
		}
		fmt.Printf("%s}\n", indent)
	}
}

func print_acessor(ac Accessor) {
	switch ac.Type {
	case ObjectField:
		fmt.Printf(".\"%s\" ", ac.Label)
		print_acessor(*ac.NextAccessor)
	case ArrayEntry:
		fmt.Printf("[%d] ", ac.Index)
		print_acessor(*ac.NextAccessor)
	case Map:
		fmt.Printf("Map ")
		print_acessor(*ac.NextAccessor)
	case End:
		fmt.Printf("End\n\n")
	}

}

/**
 * Wrapper for Printer, sends true when all threads terminate their execution
 */
func print_and_terminate(value Json, accessor Accessor, depth int, done chan<- bool) {
	println("\nResulting json value from applying Acessor: ")
	print_acessor(accessor)
	Printer(value, depth)
	done <- true
}

func main() {

	JC_channel_1 := make(chan JC)
	JC_channel_2 := make(chan JC)
	done_channel := make(chan bool)

	// Uncomment which test you wish to use and comment the other one
	// Test 1

	/* 	json_val := Json{
	   		ValueType: Array,
	   		Array: []Json{
	   			Json{ValueType: Array, Array: []Json{
	   				Json{ValueType: Boolean, Boolean: true},
	   				Json{ValueType: String, String: "NAO"},
	   				Json{ValueType: Number, Number: 123}},
	   			},
	   			Json{ValueType: Array, Array: []Json{
	   				Json{ValueType: Boolean, Boolean: true},
	   				Json{ValueType: String, String: "SIM"},
	   				Json{ValueType: Number, Number: 123}},
	   			}},
	   	}

		// Map [1] End
	   	acessor := Accessor{
	   		Type: Map,
	   		NextAccessor: &Accessor{
	   			Type:         ArrayEntry,
	   			Index:        1,
	   			NextAccessor: &Accessor{Type: End}},
	   	} */

	// Test 2
	// Uncomment which acessor you wish to use, and comment the other ones

	socialProfiles := Json{ValueType: Array, Array: []Json{
		Json{ValueType: Object, Object: map[string]Json{
			"name": Json{ValueType: String, String: "Twitter"},
			"link": Json{ValueType: String, String: "https://twitter.com"},
		}},
		Json{ValueType: Object, Object: map[string]Json{
			"name": Json{ValueType: String, String: "Facebook"},
			"link": Json{ValueType: String, String: "https://www.facebook.com"},
		}},
	}}

	languages := Json{ValueType: Array, Array: []Json{
		Json{ValueType: String, String: "Java"},
		Json{ValueType: String, String: "Node.js"},
		Json{ValueType: String, String: "JavaScript"},
		Json{ValueType: String, String: "JSON"},
	}}

	address := Json{ValueType: Object, Object: map[string]Json{
		"city":       Json{ValueType: String, String: "New York"},
		"postalCode": Json{ValueType: Number, Number: 64780},
		"Country":    Json{ValueType: String, String: "USA"},
	}}

	json_val := Json{
		ValueType: Object,
		Object: map[string]Json{
			"name":           Json{ValueType: String, String: "Jason Ray"},
			"profession":     Json{ValueType: String, String: "Software Enginner"},
			"age":            Json{ValueType: Number, Number: 31},
			"address":        address,
			"languages":      languages,
			"socialProfiles": socialProfiles,
		},
	}

	// ."socialProfiles" End
	/* 	acessor := Accessor{
		Type:         ObjectField,
		Label:        "socialProfiles",
		NextAccessor: &Accessor{Type: End},
	} */

	// ."socialProfiles" [0] End
	/* acessor := Accessor{
		Type:         ObjectField,
		Label:        "socialProfiles",
		NextAccessor: &Accessor{Type: ArrayEntry, Index: 0, NextAccessor: &Accessor{Type: End}},
	} */

	// ."socialProfiles" Map "name" End
	acessor := Accessor{
		Type:  ObjectField,
		Label: "socialProfiles",
		NextAccessor: &Accessor{
			Type: Map,
			NextAccessor: &Accessor{
				Type:  ObjectField,
				Label: "name",
				NextAccessor: &Accessor{
					Type: End}}},
	}

	// ."address" ."postalCode" End
	/* acessor := Accessor{
		Type:  ObjectField,
		Label: "address",
		NextAccessor: &Accessor{
			Type:         ObjectField,
			Label:        "postalCode",
			NextAccessor: &Accessor{Type: End}},
	} */

	// ."age" End
	/* 	acessor := Accessor{
		Type:         ObjectField,
		Label:        "age",
		NextAccessor: &Accessor{Type: End},
	} */

	// ."languages" [3] End
	/* 	acessor := Accessor{
	   		Type:  ObjectField,
	   		Label: "languages",
	   		NextAccessor: &Accessor{
	   			Type:         ArrayEntry,
	   			Index:        3,
	   			NextAccessor: &Accessor{Type: End},
	   		},
	   	}
	*/

	println("Original Json:")
	Printer(json_val, 0)

	go serialise_json(json_val, JC_channel_1)
	go eval(acessor, JC_channel_1, JC_channel_2)
	go print_and_terminate(deserialise_json(JC_channel_2), acessor, 0, done_channel)

	<-done_channel // wait for all go routines to finish
}
