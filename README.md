# boids_recreated


This is a demonstration of using the compute shader to run a simulation of flocking. Two different bind groups are used and from frame to frame we swap between using one as the src and the other as the dest, and in the next frame the previous dest is the src and the previous src is the dest. This means we can essentially use previously calculated positions for our flocking elements over time.