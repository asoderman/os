IPC message passing via shared memory design:


Name: Double Channel (?)
this diagram describes memory that is shared between both processes. E.g. Proc B has read only access to everything described under the Proc A header.
Proc A               |           Proc B
msgs written(A: r/w) | msg written (B: r/w) 
msgs read (A: r/w)   | msg read (B: r/w)
?msg size (ro)       | ?msg size (ro)
[msgs...]            | [msgs...]


Proc
