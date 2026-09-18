use crate::interpreter::Runtime;

pub fn import_tensor(rt: &mut Runtime) {
    // Load and execute the tensor.ph stdlib file
    rt.load_stdlib_file("tensor.ph");
}

