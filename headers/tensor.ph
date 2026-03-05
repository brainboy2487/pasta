# tensor.ph - placeholder for TENSOR system
# Description:
#   Definitions and stubs for tensor-related features such as
#   BUILD TENSOR, tensor.zeros/ones/rand, and other utilities.
#   The interpreter provides the underlying implementation; this
#   header simply makes the names available for import.
#
# Usage hint (human-readable):
#   # import tensor
#
# Example builtins (available after importing):
#   BUILD TENSOR: [[1,2],[3,4]]      # literal builder
#   tensor.zeros([rows, cols])
#   tensor.ones([n])
#   tensor.rand([rows, cols])
#   tensor.eye(n)                    # identity matrix
#   tensor.from_list([1,2,3])        # 1D tensor from list
#   tensor.shape(t)                  # return list of dims
#   tensor.dtype(t)                  # get dtype string
#   tensor.sum(t), tensor.mean(t)    # reductions
#   tensor.reshape(t, shape)         # change shape
#   tensor.transpose(t)              # 2D transpose
#   tensor.flatten(t)                # collapse to 1D
#
set __header_tensor = "tensor loaded"
