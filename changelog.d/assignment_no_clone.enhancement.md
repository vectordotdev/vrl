VRL programs that assign objects or arrays now run faster and allocate less: an assignment whose value
isn't used (for example, any statement that isn't the last expression of a block) no longer makes a deep
copy of the assigned value.

authors: lukesteensen
