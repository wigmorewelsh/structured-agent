
## Summary

Most of the design of agents at the moment relies on an unstructured approach to allowing them to execute. This dominates as its viewed that AI will lead to human levels of intelligence, so the model will get better over time all you need to do it put more data into the model. This does mean that the agent can do 'anything' which can act as a major security risk and may or may not execute the right process or the full process. E.g an agent may not run tests after a change.

I propose instead to adopt a structured approach based on a custom language and runtime. It will be implemented in rust and allow python fns to be exposed to the runtime.

## Language Design

Context is passed around explicitly as a special variable.

```
fn plan_work(Context) -> Result<(), Error> {
    // Implementation details here
    Ok(())
}

fn start_agent() -> {
    let agent = Context::new();
    let plan = plan_work(agent);
    // or let plan = agent.plan_work();
}
```

Adding to the context is either explicit, or using syntactic sugar implicit.

```
fn plan_work(Context) -> Result<(), Error> {
    context.Add("prompt");
    Ok(())
}

// or 
fn plan_work(Context) -> Result<(), Error> {
    "prompt"!
    Ok(())
}
```

The context is run via an LLM on function return:

```
fn start_agent() -> {
    let agent = Context::new();
    let plan = agent.plan_work();
    // plan contains the response, it conforms to the return type
}
```

```
tool thinking(Context, ) -> str {

}
```

## Implementation notes
Context is implemented as a linked list in rust with the interpreter extending the list when entering a new scope. 

## Questions
1. Should calling the LLM be implicit based on function return or explicit?
2. Could 1 be decided based on calling a function or a prompt? 

options: 
```
prompt plan_work(Context) -> SomeType {
  "prompt describing how to plan work"!;
  // LLM is executed automatically on fn return and the response is the LLM response conforming to SomeType
}

fn start_agent() -> () {
    let agent = Context::new();
    let plan = agent.plan_work();
    // LLM is not executed automatically, response is up to the developer.
}
```

alternative:
```
fn plan_work(Context) -> SomeType {
  "prompt describing how to plan work"!;
  // LLM is executed automatically on fn return and the response is the LLM response conforming to SomeType
}

fn start_agent() -> () {
    let agent = Context::new();
    let plan = agent.plan_work();
    return;
    // return is used to break out of LLM response is up to the developer.
}
```


Statements
==========
A select statement allows the LLM to pick from one or more tools.
```
fn tool_a(Context) -> ?? {
  
}

fn choose_from_tool(Context) -> () {
  select {
    case tool_a(Context, _, _): //do stuff for tool a result
    case tool_b(Context, _): //do stuff for tool b result
  }
}
```

Params can be passed in via the LLM. Using placeholder params `_`.

Minimal Buildable
-----------------
1. Functions that call LLM on return
2. Pushing text
3. Calling functions that pushes/pops state

### Minimal example

```
fn analyze_code(Context, code) -> Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}

fn suggest_fix(Context, analysis: Analysis) -> CodeFix {
    "Given this analysis, suggest a fix"!
    analysis!
}

fn main() -> () {
    let ctx = Context::new();
    
    let code = "def divide(a, b): return a / b";
    
    let analysis = ctx.analyze_code(code);
    
    let fix = ctx.suggest_fix(analysis);
}
```


# Future
* Add select statement
* Add loops
* Add return statement
* Load code from files.
* Use MCP servers as source for tools.
* Small number of built in tools (std lib).
* Run program from file.
* Programs can import from other files.
* Auto re-compile from source on changes. (salsa)
* Update running programs.
* Long running programs reacting to events.
* User input.
* Correcting failing processes. 
* Durable Execution.
* Track if events are from system(code)/user/model
* Add basic types
* Add struct and array types
* Add parsing json response to structs
