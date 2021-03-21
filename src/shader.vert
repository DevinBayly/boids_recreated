#version 450

// these are the instance values
layout (location = 0) in vec2 inPos;
layout (location = 1) in vec2 inVel;
// this is the vertex we draw with
layout (location = 2) in vec2 particle_pos;


// LOOK: out to rasterization, then to the `in` layouts in particle.frag
layout (location = 0) out vec2 outColor;

// emit a point to rasterization from each thread running particle.vert
out gl_PerVertex
{
	vec4 gl_Position;
	float gl_PointSize;
};

void main ()
{
  gl_PointSize = 2.0;
  outColor = inVel;
  gl_Position = vec4(inPos.xy + particle_pos, 1.0, 1.0);
}