# Function Signatures

## 1. Parameters

A function has a known number of parameters, these are inferred from the function signature and the module its in. These are the module params, type params, implicit modules (traits) and user params.

```sa
mod foo(s: io) 

fn reader<T: Readable>(r: T, length: Int): String {
  
}
```

The above has:
* one type param `T`
* one module param `s`
* one modular implicit `Readable`
* two user args `r: T` and `length: Int`

At runtime the function is called with all the params in a slots vector in the order Type params, Module Params, Modular Implicits (Traits), User Args. Slot 0 is empty for the return value.
[_, T, s, Readable, R, length]

At runtime this is dispatched using the CallBytecode instruction.
```rust
   CallBytecode {
        function_name: DefinitionPath,
        params: Vec<Slot>,
        dest: Slot,
    },
```

## 2. Traits

Traits declare sets of functions that form a contract for type, typically structs. Impls implement a trait for a given type.

Traits are just signatures but we can infer some information about the calling convention.

Impls are defined in modules and so have all the params that normal functions do, in addition impls also declare a `self` param. 

```sa
mod foo(s: io) 

trait Sink {
  write_to<T: Readable>(self: T, length: Int): String
}

struct Thing {}

impl Thing: Sink {
  fn write_to<T: Readable>(self, r: T, length: Int): String {
  
  }
}
```

The above impl of Sink has:
* one type param `T`
* one module param `s`
* one modular implicit `Readable`
* three user args `self: Thing`, `r: T` and `length: Int`

The param order for calls is the same as normal functions, but with `self` as the first user arg:
[_, T, s, Readable, self: Thing, R, length]

Just like function calls at runtime this is dispatched the CallBytecode instruction, there is no difference.

## 3. Dispatching to generic traits

One area that need mentioning is calls on generic types, because the implementation can change for different types we have to do dispatch differently. However the implementation of the trait that we dispatch to still follows all the same rules. 

```sa
mod foo(s: io) 

trait Sink {
  write_to<T: Readable>(self: T, length: Int): String
}

struct Thing {}

impl Thing: Sink {
  fn write_to<T: Readable>(self, r: T, length: Int): String {
  
  }
}

fn do_thing<T: Sink>(x: T) {
  x.write_to(10, ReadableImpl{})
}

do_thing(Thing{})
```

Because T isn't known at compile time the params for the function are split in two. The Type params, Module params, Modular Implicits (traits) are passed into the function at runtime using a Module data type, while the user params are handled by the VM as usual. The runtime uses a separate dispatch instruction `CallIndirect` which it takes the module to bind T to as a param an looks up the function to call at runtime.

NOTE: I'm not convicted that this is implemented correctly, as we are passing in the module and its params but I think we are missing the types and modular implicits. The implementation need to be reviewed. In fact I'm sure this is wrong.

```rust
    Module {
        path: DefinitionPath,
        params: Vec<ExpressionValue>,
    },
```

```rust
    CallIndirect {
        module_param: Slot,
        fn_name: DefinitionPath,
        params: Vec<Slot>,
        dest: Slot,
    },
```

## 4. Dispatching to actors

So actors in SA are statically typed as they are themselves modules. Functions still take the same params as normal functions, the only difference is at runtime all the args are pushed onto a tokio channel and the result is received over another channel.

## 5. Module param resolution



## Stupid things you can do with SA modules

### Reexport an existing module then override it in a use statement.

```sa
mod Backing {
    fn other_fun(): () {}
}

mod Exporter(e: Exportee) {
	pub use e as exportee
}

mod Exportee(e: Backing) {
	fn some_fun(): () {}
}

mod Override(e: Backing) {
	fn some_fun(): () {}
}

use Exporter(Override)::exportee(Backing)::some_fun
```

how do you resolve the above? 
the type checker has to resolve the `Override` module first, 
then resolve the Backing module, 
using both the modules it can then resolve exportee
which is the module param so exportee resolves to Override.
Then it can resolve some_fun which is a function in Override.



### Implement a trait for a module

```sa
trait Test {
	fn test(self): ()
}

mod SomeModule {
	fn do_work(): () {}
}

impl SomeModule: Test {
    fn test(self): () {
        self.do_work()
    }
}
```